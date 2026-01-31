# Architecture

- [Overview](#overview)
- [State Machines](#state-machines)
  - [Client State Machine](#client-state-machine)
    - [Lobby](#lobby)
  - [Server State Machine](#server-state-machine)
- [File structure](#file-structure)

# Overview

The game uses a client-server architecture. In Rust terms, these are represented by two separate packages: `server` and `client`. Both depend on a third package, `common`, for shared types, physics, and communication protocol.

## State Machines

Both client and server use the state pattern to organize flow. Each has its own collection of states.

### Client State Machine

Top-level states:

```txt
Lobby -> Game -> AfterGameChat -> EndAfterLeaderboard
```

- `Transitioning` is a formal state used only during the transition from `Game` to `AfterGameChat`. The run loop replaces the current state with `Transitioning` (via `ClientState::default()`) so it can consume the `Game` state to build the full `AfterGameChat` value, then immediately sets the state to `AfterGameChat`. The client is in `Transitioning` only for that brief moment; the run loop does nothing while in this state.
- `Disconnected` can be entered from the Lobby substate `Connecting` onwards (and from Game/AfterGameChat on connection loss).
- `EndAfterLeaderboard` is terminal: the client displays the post-game leaderboard and waits for the player to exit.

Lobby has various substates, as detailed [below](#lobby).

#### Lobby

```
ServerAddress -> Passcode -> Connecting -> Authenticating -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat
```

`ServerAddress` prompts for an IP address and port number; pressing Enter uses the local default.

If the player is the host: `Chat -> ChoosingDifficulty`, then the host starts the countdown and everyone (including the host) receives `CountdownStarted` and enters `Countdown`. Non-hosts: `Chat -> Countdown` when the server broadcasts that the countdown has started. In either case,

```txt
Countdown -> Game
```

### Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

- The host, in Lobby, triggers a move to `ChoosingDifficulty`; when the host starts the game, the server moves to `Countdown` and broadcasts to all clients.
- The server enters the formal state `Exiting` only from `Game`: when the leaderboard has been sent to all clients in after-game chat (`leaderboard_sent`), the game handler returns `Exiting` and the run loop breaks, exiting the process. If all clients disconnect during `Game` (before or after entering after-game chat), the server does not transition to `Exiting`; instead `Game::remove_client` calls `std::process::exit(0)` when the last client is removed, so the process exits without ever entering `Exiting`. (TODO: Explain why!) If all clients disconnect during `Lobby`, `ChoosingDifficulty`, or `Countdown`, the server does not move to `Exiting` and does not exit; it remains in that state with zero clients. (TODO: Fix this!)
- The `Game` state also manages clients in after-game chat, since they arrive at different times.

## File structure

### Server

```txt
server/src/
├── lib.rs
├── main.rs
├── input.rs
├── net.rs
├── player.rs
├── run.rs
├── state.rs
├── state_handlers.rs
├── state_handlers/
│   ├── countdown.rs
│   ├── difficulty.rs
│   ├── game.rs
│   └── lobby.rs
└── test_helpers.rs
```

### Client

```txt
client/src/
├── lib.rs
├── main.rs
├── after_game_chat.rs
├── assets.rs
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
│   │   ├── after_game.rs
│   │   ├── initialize.rs
│   │   └── update.rs
│   └── map.rs
├── info.rs
├── lobby/
│   ├── flow.rs
│   ├── state_handlers/
│   │   ├── auth.rs
│   │   ├── chat.rs
│   │   ├── connecting.rs
│   │   ├── countdown.rs
│   │   ├── difficulty.rs
│   │   ├── passcode.rs
│   │   ├── server_address.rs
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
├── run.rs
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
│   │   │   ├── prim.rs
│   │   │   └── wilson.rs
│   │   └── algorithms.rs
│   └── maker.rs
├── maze.rs
├── net.rs
├── player.rs
├── protocol.rs
├── ring.rs
├── snapshot.rs
└── time.rs
```
