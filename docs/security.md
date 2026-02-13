# Security

Elements:

- Matchmaker
- Client
- Game servers (launched dynamically by the matchmaker)

- Reverse proxy (Caddy)[^1]
- Docker socket proxy (tecnativa/docker-socket-proxy).

By matchmaker, here, I just mean a program for launching games to be played among groups of friends, rather than a matchmaker in the strict sense of matching strangers.

Codes:

- Version code
- Passcode
- Private key
- Connect token

Matchmaker and game server run in Docker containers on my VPS, likewise Caddy and the Docker socket proxy.[^2]

The client is the frontend, downloaded by players. When a player starts the client, it gives them a choice:

- New game
- Join game

If the player picks new game, they're asked for a username and how many players will play. The client then sends a HTTPS request, `POST /games`, to `api.by-a-thread.de`, including these details and a baked-in version code to prove that it is indeed a client for this game, and, in particular, for the current version. This request is intercepted by Caddy and forwarded by HTTP to the matchmaker. In fact, all communication between clients and matchmaker is mediated by Caddy.

The matchmaker checks the version code against a hash. If the hashes match, it checks how many games are in progress, how many players have been assigned to each game, and current CPU usage. If all are within the set limits, the matchmaker creates a new game.[^3] It picks a port number from a pool (7777–7782), then generates a passcode, consisting of six random digits, and a longer private key, consisting of 32 random bytes.

The private key is required by the Renet networking library to allow secure communication via the UDP protocol between clients and game servers. The matchmaker uses the private key to generate a unique connect token for every player. It then launches a game server in a new Docker container via the Docker socket proxy, passing the private key to the game server as an environment variable. If that succeeds, the matchmaker responds to the client with the port number, connect token, and passcode, and starts a 'lobby timer'. If it fails, the matchmaker responds with an appropriate error. See the [API spec](api.yaml) for request and response formats and error codes.

The matchmaker will also rate limit new-game requests to one per 30s. (This can be turned off while testing.)

We'll refer to the player who initiated the game as the host. They share the passcode, out of band, with other players. The host client automatically connects to the game server, using the connect token and port. The game server marks them as the host.

If a player receives the passcode, they can choose "join game", which sends a HTTPS request, `POST /games/{passcode}/join` to `api.by-a-thread.de`, including their name, passcode, and the version code. If these credentials are valid, and the lobby timer has not elapsed, the matchmaker responds with a connect token and the port number. See the [API spec](api.yaml) for details. The client uses these to connect automatically to the game server.

When the lobby timer has run out, the matchmaker will no longer issue connect tokens. Existing tokens remain valid until they expire. The timer will be shown to players in the GUI so that they know how long they have to start the game. They wait in a chat room. The game proper will begin when the host initiates it or the timer expires.

The game itself has a timer of ten minutes for multiplayer games, and two for single-player games.

When players die, they return to the chat room. When the game is over for everyone, they can chat for another five minutes. The server will exit when that last timer expires or when all clients have disconnected, whichever is first.

## Footnotes

[^1]: Caddy also takes care of TLS certificates, renewing them as needed.

[^2]: The matchmaker's access to Docker must always go through the Docker socket proxy. An attacker who finds a vulnerability in the matchmaker could otherwise launch a privileged container and thereby gain root access to the host. The raw Docker socket will be mounted into the proxy, which can accept desired commands (like `start container`) and block dangerous ones (like `mount volume` or `delete system`).

[^3]: For now, I'm just allowing five games of ten players each, but I may fine-tune that in the future to restrict new games if CPU usage is high, or allow more games if they have fewer players.
