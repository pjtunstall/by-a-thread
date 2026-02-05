# Security

- [Development](#development)
- [Production](#production)

## Development

Currently, as a shortcut during development, the client imports (what should be) a private key from the `common` package and uses it to create the token needed to establish a Renet connection with the server. The server logs a random passcode to the terminal, different each game. This can be shared with any players who want to join the game. The first to join is designated the host, which just means they get to choose the difficulty level, triggering the start of the game itself.

## Production

Clearly this is not sufficient for a public game. In production, my plan is to have create a matchmaker that will be responsible for managing game sessions.[^1] It will launch game servers in response to HTTP requests and clean them up when they are no longer needed.

A would-be host will request a game from the matchmaker via HTTPS. If a slot is available (i.e. not too many games or players are already playing), the matchmaker will create two ephereral (i.e. per game) random secrets for the game server: a private key and a reporting token. The private key is for the game server to decrypt messages from clients. The reporting token is for the game server to identify itself when it reports back to the matchmaker. The matchmaker will launch an instance of the game server in a dedicated container,[^2] passing these secrets to it privately on HTTP via a Docker bridge network.

Meanwhile, the matchmaker will generate a connect token from the private key and pass this, along with the game's port number and an ephemeral passcode, unique to the game, to the host.

When the host receives this data, they will automatically connect to the game server using the connect token and port number. As this client is the first player to connect, the server will mark them as the host. The host can then share the passcode with friends.

Now the other players can send the passcode to the matchmaker via HTTPS. If it's valid, the matchmaker will reply with the connect token and port. They'll use these to connect to the game server, which will admit them provided the token is valid, there's less than the maximum number of players, and the game has not begun yet.

When the host has chosen a difficulty level and sent it to the game server, latter will report to the matchmaker via the Docker bridge network, identifying itself with the reporting token. This will allow the matchmaker to update the game's status to "in progress". Once the matchmaker has sent an acknowledgement back to the server, the latter can proceed with the game proper.

[^1]: For the purposes of this document, the matchmaker is just a program for launching games to be played among groups of friends, rather than a matchmaker in the strict sense of matching strangers.

[^2]: The matchmaker's access to Docker will be mediated by a Docker socket proxy. This is because an attacker who finds a vulnerability in the matchmaker could launch a privileged container and thereby gain root access to the host. The raw Docker socket will be mounted into the proxy, which can accept desired commands (like `start container`) and block dangerous ones (like `mount volume` or `delete system`).
