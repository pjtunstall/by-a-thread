# Security

Key elements:

- Client
- Matchmaker
- Game servers (launched dynamically by the matchmaker)
- Reverse proxy (Caddy)[^1]
- Docker socket proxy (tecnativa/docker-socket-proxy).

By matchmaker, here, I just mean a program for launching games to be played among groups of friends, rather than a matchmaker in the strict sense of matching strangers.

Codes:

- Client proof (baked-in secret proving the request is from a real game client)
- Passcode
- Private key
- Connect token (must remain valid for the entire session: lobby, countdown, and game)

Matchmaker and game server run in Docker containers on my VPS, likewise Caddy and the Docker socket proxy.[^2]

The client is the frontend, downloaded by players. When a player starts the client, it gives them a choice:

- New game
- Join game

If the player picks new game, they're asked how many players will play. The client then sends a HTTPS request, `POST /games`, to `api.by-a-thread.de`.[^3] The request includes the player count and two headers: a client proof (baked-in base64 secret) and the client's version string. This request is intercepted by Caddy and forwarded by HTTP to the matchmaker. In fact, all communication between clients and matchmaker is mediated by Caddy.

For both new-game and join-game requests, the matchmaker validates the request before handling it. It requires both headers. The client proof is decoded from base64, hashed with SHA-256, and compared in constant time to a configured hash; the version string must equal the version the matchmaker was built with. If either check fails, the matchmaker responds with an error (for a version-string mismatch, the body tells the user to download the current version). If both pass, for a new-game request the matchmaker checks how many games are in progress (and potentially in the future also how many players have been assigned to each game and current CPU usage). If all are within the set limits, the matchmaker creates a new game.[^4] It picks a port number from a pool of ten (7777–7786), then generates a passcode, consisting of six random digits, and a longer private key, consisting of 32 random bytes.

The private key is required by the Renet networking library to allow secure communication via the UDP protocol between clients and game servers. The matchmaker uses the private key to generate a unique connect token for every player. The connect token must remain valid for the entire duration of the client's connection--not just during the initial handshake--because the netcode layer validates it continuously. If a token expires mid-game, the client is disconnected. It then launches a game server in a new Docker container via the Docker socket proxy, passing the private key to the game server as an environment variable. If that succeeds, the matchmaker responds to the client with the port number, connect token, and passcode. The matchmaker tracks active games along with a record of when each one started. If Docker fails to start the game server, the matchmaker responds to the client with an appropriate error. See the [API spec](api.yaml) for request and response formats and error codes.

The matchmaker will also rate limit new-game requests. (This can be turned off while testing.)

The host is the first player whose username is confirmed in the lobby (the first to join the chat); in practice this is usually the player who created the game, since they connect first. They share the passcode, out of band, with other players. The host client automatically connects to the game server, using the connect token and port. The game server marks them as the host.

If a player receives the passcode, they can choose "join game", which sends a HTTPS request, `POST /games/{passcode}/join` to `api.by-a-thread.de`, with the same two headers (client proof and version string) as for new-game. The matchmaker applies the same validation; if it passes and the passcode is valid and no more than five minutes has passed since the game server started, the matchmaker responds with a connect token and the port number. Players send their usernames to the game server after connecting; the server checks uniqueness. The client uses these to connect automatically to the game server.

After five minutes, the matchmaker will no longer issue connect tokens. The lobby timer is shown to players in the GUI so that they know how long they have to start the game. They wait in a chat room. The game proper begins when the host initiates it or five minutes have elapsed. Connect tokens are issued with an expiry that exceeds the maximum possible session length (lobby + countdown + full game), so that tokens remain valid for the entire time clients are connected.

The game itself has a timer of ten minutes for multiplayer games, and two for single-player games.

When players die, they return to the chat room. Likewise when the in-game timer expires or the game is over. When the game is over, the server sends a leaderboard to all clients as a `Reliable` Renet message. The server then exits. Players are shown a final message after the leaderboard, and are offered the choice to exit or play again.

As a safety measure, in case game server containers are left running if the matchmaker crashes, it cleans up any existing ("zombie") game server containers when it starts.

## Footnotes

[^1]: Caddy also takes care of TLS certificates, renewing them as needed.

[^2]: The matchmaker's access to Docker must always go through the Docker socket proxy. An attacker who finds a vulnerability in the matchmaker could otherwise launch a privileged container and thereby gain root access to the host. The raw Docker socket will be mounted into the proxy, which can accept desired commands (like `start container`) and block dangerous ones (like `mount volume` or `delete system`).

[^3]: See the [API spec](api.yaml) for details.

[^4]: For now, I'm allowing ten games of ten players each, but I may fine-tune that in the future to restrict new games if CPU usage is high, or allow more games if they have fewer players.
