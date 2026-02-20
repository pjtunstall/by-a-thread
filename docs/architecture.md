# Architecture

- [Overview](#overview)
- [Matchmaker and deployment](#matchmaker-and-deployment)
  - [Running the stack](#running-the-stack)
  - [Environment files](#environment-files)
  - [Local development](#local-development)
  - [Deployment](#deployment)
- [State Machines](#state-machines)
  - [Client State Machine](#client-state-machine)
    - [PreLobby](#prelobby)
    - [Lobby](#lobby)
  - [Server State Machine](#server-state-machine)
- [File structure](#file-structure)

# Overview

The game uses a client-server architecture. In Rust terms, these are represented by two separate packages: `server` and `client`. Both depend on a third package, `common`, for shared types, physics, and communication protocol.

Before connecting to a game server, the client calls a matchmaker REST API to create or join a session. The matchmaker allocates ports, generates passcodes, and launches game servers in Docker. See [Matchmaker and deployment](#matchmaker-and-deployment).

## Matchmaker and deployment

The matchmaker is a REST API that creates and joins game sessions. It checks limits (games, and potentially in the future also total player count and CPU usage), allocates ports, generates passcodes and private keys, and launches game servers as Docker containers. The API spec is in [api.yaml](api.yaml).

The stack runs via Docker Compose:

- **matchmaker**: HTTP server (Axum) on port 8080. Receives create-game and join-game requests, validates version codes, enforces rate limiting, and spawns game server containers via the Docker API.
- **caddy**: Reverse proxy and TLS termination. Exposes ports 80 and 443; proxies `api.by-a-thread.de` to the matchmaker. Handles Let's Encrypt certificates in production.
- **socket-proxy** ([tecnativa/docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy)): Exposes a restricted subset of the Docker socket to the matchmaker. The matchmaker connects to `tcp://socket-proxy:2375` instead of the raw socket. The proxy allows only `CONTAINERS=1`, `POST=1`, and `NETWORKS=1`, so the matchmaker can create containers and attach them to networks but cannot list or inspect other containers.

All three services share a backend bridge network. Caddy reverse-proxies the matchmaker; the matchmaker connects to the socket proxy to run Docker. The matchmaker reads `.env.matchmaker` for `VERSION_HASH`, `EXPECTED_VERSION`, `HOST`, and related config. See [Docker](docker.md) for compose stack details.

### Running the stack

Build the game server image with `make server` (builds the server binary and Docker image `server-image:latest`). Start the compose stack with `docker compose up -d`. After changes to the matchmaker source, run `docker compose up -d --build` to rebuild and restart the matchmaker container.

### Environment files

Create `.env.client` and `.env.matchmaker` in the project root. The client is built with values from `.env.client`; the matchmaker container reads `.env.matchmaker`. Run `make check-env` to ensure the two files are consistent.

**.env.client** (used at client build time; values will be baked into the binary on build):

- `VERSION_CODE` (required): Base64-encoded secret. The matchmaker validates the client by checking that the SHA-256 hash of the decoded bytes equals `VERSION_HASH` in `.env.matchmaker`. The secret can be any length; 32 bytes is a typical choice (44 base64 characters). Generate a pair once and use the same `VERSION_CODE` in `.env.client` and the corresponding hex `VERSION_HASH` in `.env.matchmaker`.

The Renet protocol id is derived from the git commit hash at build time (when available), so client and server built from different commits will reject each other with a clear error. Rebuild both from the same source to fix protocol mismatches.

- `HOST` (optional): Default server for API and game. Omit or leave empty for local. For production, set to your domain (e.g. `HOST=by-a-thread.de`). Must match `HOST` in `.env.matchmaker`.

**.env.matchmaker** (loaded by Docker Compose for the matchmaker service):

- `VERSION_HASH` (required): 64-character hex string (32 bytes), the SHA-256 hash of the bytes that are base64-encoded as `VERSION_CODE` in `.env.client`.
- `EXPECTED_VERSION` (required): Semantic version string (e.g. `0.1.0`) that must match the matchmaker build version (`common` crate `CARGO_PKG_VERSION`). The matchmaker panics on startup if they differ. Set it to the version of the matchmaker image you are running.
- `HOST` (optional): Same meaning as in `.env.client`. Omit or set to `local` or `localhost` for local; set to your domain for production. Must match `.env.client` (see `make check-env`).

**Local vs production:**

|  | Local | Production |
| --- | --- | --- |
| `.env.client` | Leave `HOST` commented out or unset. | Set `HOST=your-domain.de`. |
| `.env.matchmaker` | Leave `HOST` commented out or set to `local`. | Set `HOST=your-domain.de`. |
| `.env` | Should contain `CADDYFILE=./Caddyfile.local`. | Omit to use default Caddyfile. |

After changing `HOST` for production, rebuild the client so the new default server is baked in.

### Local development

To run locally, extra care is needed for Caddy and Docker:

**Caddy**

1. The API host for local requests must be `localhost`, not `127.0.0.1`, or Caddy rejects the request. When connecting to an IP address, the TLS ClientHello has an empty Server Name Indication (SNI); Caddy refuses connections without SNI because it cannot match the request to a certificate. Using `localhost` sends the hostname in SNI, so Caddy can match the site block.

2. In production, Caddy uses the default Caddyfile which tells it to obtain trusted TLS certificates from Let's Encrypt for `api.by-a-thread.de`. For local development, that's not possible. Create a `.env` file with `CADDYFILE=./Caddyfile.local` so that Caddy uses Caddyfile.local instead of Caddyfile. Caddyfile.local tells Caddy to use self-signed certs instead of trying to get them from Let's Encrypt. For the sake of local testing, our client accepts these over localhost, and only over localhost.

**Docker**

If the client tries to connect on 127.0.0.1, the server can't reply. When it tries to, it misinterprets the client's 127.0.0.1 (i.e. the address of the host OS) with its own in-container loopback address, and sends the reply to itself.

- Client (on host) sends to 127.0.0.1:7785 from something like 127.0.0.1:45678.
- Docker receives it on the host and DNATs the destination to the container's 5000.
- The source address is usually left as 127.0.0.1:45678.
- The server in the container does recv_from() and sees source 127.0.0.1:45678.
- The server sends the reply to 127.0.0.1:45678.
- Inside the container, 127.0.0.1 is the container's own loopback. The reply goes to the container's loopback, not the host.
- The host client never gets the reply, so the connection times out.

The solution is to make the client connect to the default Docker bridge network, 172.17.0.1. This is the host OS's address as understood both inside and outside the server container.

- Client sends to 172.17.0.1:7785 from something like 172.17.0.1:45678 (same interface).
- Docker forwards to the container; the server sees source 172.17.0.1:45678.
- The server sends the reply to 172.17.0.1:45678.
- From the container, 172.17.0.1 is the gateway to the host. The reply goes out to the host.
- The host receives it on 172.17.0.1 and delivers it to the client.
- The client gets the reply and the connection succeeds.

### Deployment

Brief outline for running the stack on a cloud server:

1. **Server and Docker**  
   Provision a cloud server (e.g. Hetzner). Install Docker; the Compose plugin is included with current Docker (use `docker compose`). No separate Compose install is needed.

2. **SSH**  
   Set up SSH keys and ensure you can log in. The Makefile deploy target uses the SSH alias `hetzner` (see below); any suitable host alias in `~/.ssh/config` can be used instead.

3. **Firewall**  
   Allow SSH (22), HTTP (80), HTTPS (443), and UDP on the 10 game server ports 7777–7786.

4. **Deploy**  
   Run `make deploy-hetzner` after a full `make` (or at least `make server`). This builds and pushes the latest game server Docker image to the VPS and runs a single server container there. It does **not** yet deploy the matchmaker stack (matchmaker, Caddy, socket-proxy) or use Docker Compose on the server; bringing up the full compose stack on the VPS is currently a separate, manual step (copy `docker-compose.yaml`, `.env.matchmaker`, and Caddyfile; run `docker compose up -d` on the server). This will be automated in due course.

## State Machines

Both client and server use the state pattern to organize flow. Each has its own collection of states.

### Client State Machine

Top-level states:

```txt
PreLobby -> Lobby -> Game -> PostGameChat -> EndAfterLeaderboard
```

- `PreLobby` runs before the client establishes a network connection. It is driven by `run_pre_lobby_loop`; the main run loop (`ClientRunner`) does not exist yet. On completion, the client transitions to `Lobby::Connecting` and enters the run loop.
- `Transitioning` is a formal state used only during the transition from `Game` to `PostGameChat`. The run loop replaces the current state with `Transitioning` (via `ClientState::default()`) so it can consume the `Game` state to build the full `PostGameChat` value, then immediately sets the state to `PostGameChat`. The client is in `Transitioning` only for that brief moment; the run loop does nothing while in this state.
- `Disconnected` can be entered from the Lobby substate `Connecting` onwards (and from Game/PostGameChat on connection loss). The player can press ESCAPE to exit or ENTER to return to the start menu (PreLobby).
- `EndAfterLeaderboard` is terminal: the client displays the post-game leaderboard. The player can press ESCAPE to exit or ENTER to return to the start menu (PreLobby).

`PreLobby` and `Lobby` have various substates, as detailed [below](#prelobby) and [below](#lobby).

#### PreLobby

```
ServerAddress -> ApiRequestMenu
```

- `ServerAddress`: the player enters the matchmaker API host (or uses default). The client pings the host to verify reachability before proceeding.
- `ApiRequestMenu`: the player chooses "New game" or "Join game", then either enters player count (new) or passcode (join). The client calls the matchmaker API; the request runs in a background thread so the UI stays responsive and the player can press Escape to cancel. On success, the client receives a connect token and transitions to `Lobby::Connecting`.

`ApiRequestMenu` internal phases: `ChoosingNewOrJoin` -> `ChoosingPlayerCount` (new) or `ChoosingPasscode` (join) -> `AwaitingCreate` or `AwaitingJoin` (polling the API response).

#### Lobby

```
Connecting -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat

host: Chat -> ChoosingDifficulty -> Countdown -> Game
non-host: Chat -> Countdown -> Game
```

The client connects using the connect token obtained in PreLobby. If the player is the host: `Chat -> ChoosingDifficulty`, then the host starts the countdown and everyone (including the host) receives `CountdownStarted` and enters `Countdown`. Non-hosts: `Chat -> Countdown` when the server broadcasts that the countdown has started. In either case,

```txt
Countdown -> Game
```

### Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

- The host, in Lobby, triggers a move to `ChoosingDifficulty`; when the host starts the game, the server moves to `Countdown` and broadcasts to all clients.
- The server enters the formal state `Ending` only from `Game`: when the leaderboard has been sent to all clients in after-game chat (`leaderboard_sent`), the game handler returns `Ending` and the run loop breaks, exiting the process.
- If all clients disconnect during `Game`, `Lobby`, or `ChoosingDifficulty`, the server does not transition to `Ending`; instead the given state's `remove_client` method calls `std::process::exit(0)` when the last client is removed, so the process exits. (This only runs when a client actually disconnects, so the server does not exit at startup when no one has connected.) If all clients disconnect during `Countdown`, the server just waits for the game to start, and lets the disconnection logic there take care of it.
- If no client connects within one minute of startup, the server exits. This failsafe avoids leaving orphaned containers running when connection fails.
- If the server is waiting for clients to send `EnterPostGameChat` (ready for leaderboard) and some do not within 6 seconds, the server sends the leaderboard anyway and exits.
- The `Game` state also manages clients in after-game chat, since they arrive at different times.

## File structure

### Repository root (deployment)

```txt
.
├── docker-compose.yaml
├── Caddyfile
├── Caddyfile.local
├── .env.matchmaker
└── docs/
    ├── architecture.md
    ├── api.yaml
    └── docker.md
```

### Matchmaker

```txt
matchmaker/
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── addressing.rs
│   ├── auth.rs
│   ├── cleanup.rs
│   ├── errors.rs
│   ├── extractors.rs
│   ├── game.rs
│   ├── games.rs
│   ├── handlers/
│   │   ├── create.rs
│   │   └── join.rs
│   ├── handlers.rs
│   ├── ports.rs
│   ├── rate_limiter.rs
│   └── state.rs
├── Cargo.toml
└── Dockerfile
```

### Server

```txt
server/
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── input.rs
│   ├── net.rs
│   ├── player.rs
│   ├── run.rs
│   ├── state.rs
│   ├── state_handlers.rs
│   ├── state_handlers/
│   │   ├── countdown.rs
│   │   ├── difficulty.rs
│   │   ├── game.rs
│   │   └── lobby.rs
│   └── test_helpers.rs
└── tests/
    └── chat.rs
```

### Client

```txt
client/src/
├── lib.rs
├── main.rs
├── post_game_chat.rs
├── assets.rs
├── api.rs
├── config.rs
├── exit.rs
├── fade.rs
├── frame.rs
├── game/
│   ├── input.rs
│   ├── obe.rs
│   ├── state.rs
│   ├── victory.rs
│   ├── world/
│   │   ├── avatar.rs
│   │   ├── bullet.rs
│   │   ├── maze.rs
│   │   └── sky.rs
│   └── world.rs
├── game.rs
├── info/
│   ├── circles.rs
│   ├── crosshairs.rs
│   ├── map/
│   │   ├── post_game.rs
│   │   ├── initialize.rs
│   │   └── update.rs
│   └── map.rs
├── info.rs
├── lobby/
│   ├── flow.rs
│   ├── state_handlers/
│   │   ├── chat.rs
│   │   ├── connecting.rs
│   │   ├── countdown.rs
│   │   ├── difficulty.rs
│   │   ├── start_countdown.rs
│   │   ├── username.rs
│   │   └── waiting.rs
│   ├── state_handlers.rs
│   ├── state.rs
│   ├── ui/
│   │   └── gui.rs
│   └── ui.rs
├── lobby.rs
├── net.rs
├── pre_lobby/
│   ├── flow.rs
│   ├── state_handlers/
│   │   ├── api_request_menu.rs
│   │   └── server_address.rs
│   ├── state_handlers.rs
│   └── state.rs
├── pre_lobby.rs
├── run.rs
├── server_address.rs
├── session.rs
├── state.rs
├── test_helpers.rs
└── time.rs
```

### Common

```txt
common/src/
├── lib.rs
├── auth.rs
├── bullets.rs
├── chat.rs
├── constants.rs
├── input.rs
├── maze/
│   ├── maker/
│   │   ├── algorithms/
│   │   │   ├── backtrack.rs
│   │   │   ├── binary_tree.rs
│   │   │   ├── blobby.rs
│   │   │   ├── division.rs
│   │   │   ├── kruskal.rs
│   │   │   ├── prim.rs
│   │   │   ├── twiggy.rs
│   │   │   ├── wilson.rs
│   │   │   └── algorithms.rs
│   │   └── maker.rs
│   └── maze.rs
├── net.rs
├── player.rs
├── protocol.rs
├── ring.rs
├── snapshot.rs
└── time.rs
```
