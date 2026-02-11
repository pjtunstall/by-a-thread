# Security

- [Development](#development)
- [Production](#production)

## Development

I've used a simplified security system as a placeholder during development. The client imports (what should be) a private key from the `common` package and uses it to create the token needed to establish a Renet connection with the server. The server logs a random passcode to the terminal, different each game. This can be shared with any players who want to join the game. The first to join is designated the host, which just means they get to choose the difficulty level, triggering the start of the game itself.

While players are joining, the others can chat in a lobby.

If the host disconnects, another player is promoted to host. The server exits if all players disconnect, there's been no activity in a lobby phase for five minutes, or the game has been played and ended naturally.

## Production

For production, my plan is to create a matchmaker that will be responsible for managing game sessions. (By matchmaker, here, I just mean a program for launching games to be played among groups of friends, rather than a matchmaker in the strict sense of matching strangers.)

A would-be host will request a game for _n_ players from the matchmaker via HTTPS.[^1] The matchmaker will check if it can grant the request based on limits on the number of existing players, games, and CPU-usage. If a slot is available, the matchmaker will pick a port number from a pool, then generate a short, random passcode and a longer private key, as required by the Renet library, for encrypted and authenticated communication during the game. Then it will launch a new game server in a Docker container, supplying it with the private key as an environment variable.[^2] Finally, the matchmaker will generate _n_ connect tokens from the private key and send one to the host along with the passcode and port number.

When the host receives this data, they will automatically connect to the game server using the connect token and port number. As this client is the first player to connect, the server will mark them as the host. The host can then share the passcode with friends.

Now the other players can send the passcode to the matchmaker via HTTPS. If it's valid, and if there are still connect tokens left for this game, the matchmaker will reply with the connect token and port. They'll use these to connect to the game server, which will admit them provided the token is valid and the game has not begun yet.

The lobby phase of the game will have a time limit, after which the matchmaker will no longer hold onto unclaimed connect tokens. The timer will be shown to players in the GUI so that they know how long they have to start the game. The game proper will begin when the host initiates it or the timer expires.

The game itself has a timer. Currently the server exits when it ends and the leaderboard has been shown. I might let it continue a few minutes more for players to chat in the after-game lobby.

## Footnotes

[^1]: Via a reverse proxy, Caddy, which will also take care of TLS certificates.

[^2]: The matchmaker's access to Docker will be mediated by a Docker socket proxy. This is because an attacker who finds a vulnerability in the matchmaker could launch a privileged container and thereby gain root access to the host. The raw Docker socket will be mounted into the proxy, which can accept desired commands (like `start container`) and block dangerous ones (like `mount volume` or `delete system`).
