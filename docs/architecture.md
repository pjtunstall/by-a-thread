# Architecture

- [Overview](#overview)
- [Matchmaker and deployment](#matchmaker-and-deployment)
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

- **matchmaker**: HTTP server (Axum) on port 8080. Receives create-game and join-game requests, validates client proof, enforces rate limiting, and spawns game server containers via the Docker API.
- **caddy**: Reverse proxy and TLS termination. Exposes ports 80 and 443; proxies `api.by-a-thread.de` to the matchmaker. Handles Let's Encrypt certificates in production.
- **socket-proxy** ([tecnativa/docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy)): Exposes a restricted subset of the Docker socket to the matchmaker. The matchmaker connects to `tcp://socket-proxy:2375` instead of the raw socket. The proxy is configured with `CONTAINERS=1` and `POST=1`, which means the matchmaker can perform container operations (including listing, inspecting, creating, and removing its game server containers) but cannot access other Docker API groups such as volumes or networks. Hardening against privileged containers and host mounts is handled by the Docker authorization plugin and OPA policy described in `security.md`.

Caddy, matchmaker, and socket-proxy are attached to two user-defined Docker bridge networks declared in `docker-compose.yaml`: a `front` network (Caddy ↔ matchmaker) and a `back` network (matchmaker ↔ socket-proxy and game servers). Caddy reverse-proxies the matchmaker over `front`; the matchmaker connects to the socket proxy and game servers over `back`. The matchmaker reads `.env.matchmaker` for `CLIENT_PROOF_HASH`, `HOST`, and related config. The version it expects from clients is its own Cargo package version, baked in at build time via `env!("CARGO_PKG_VERSION")` (there is no env var for it). The client sends its own package version in the `X-Version` header; the matchmaker rejects the request if they differ. In this workspace all crates use the same workspace version, so a client built from the same tree matches. For more detail see [devops.md](#devops.md).

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

The host is the first player whose username is confirmed in the lobby (the first to join the chat); in practice this is likely to be the player who created the game, since they connect first. The client connects using the connect token obtained in PreLobby. If the player is the host: `Chat -> ChoosingDifficulty`, then the host starts the countdown and everyone (including the host) receives `CountdownStarted` and enters `Countdown`. Non-hosts: `Chat -> Countdown` when the server broadcasts that the countdown has started. In either case,

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
├── .env
├── .env.client
└── .env.matchmaker
```

For what to put in the `.env` files, see [devops.md](#devops.md).

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
|   |   ├── health.rs
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
├── domain.rs
├── input.rs
├── maze/
│   ├── maker/
│   │   ├── algorithms/
│   │   │   ├── backtracker.rs
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
