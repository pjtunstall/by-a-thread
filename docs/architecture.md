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

```txt
Lobby -> Game -> AfterGameChat
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state (or substate) can lead to `Disconnected`.

#### Lobby

```
ServerAddress -> Passcode -> Connecting -> Authenticating -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat
```

`ServerAddress` prompts for an IP address and port number; pressing Enter uses the local default.

If the player is the host: `Chat -> ChoosingDifficulty`, otherwise `Chat -> Countdown`. In either case,

```txt
Countdown -> Game
```

### Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

The `Game` state also manages clients in `AfterGameChat` since they arrive at different times.

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
├── game.rs
│   ├── input.rs
│   ├── obe.rs
│   ├── state.rs
│   └── world.rs
│       ├── avatar.rs
│       ├── bullet.rs
│       ├── maze.rs
│       └── sky.rs
├── info.rs
│   └── (4 files)
├── lobby.rs
│   ├── flow.rs
│   ├── state.rs
│   ├── state_handlers.rs
│   ├── ui.rs
│   └── state_handlers/
│       ├── auth.rs
│       ├── chat.rs
│       ├── connecting.rs
│       ├── countdown.rs
│       ├── difficulty.rs
│       ├── passcode.rs
│       ├── server_address.rs
│       ├── start_countdown.rs
│       ├── username.rs
│       └── waiting.rs
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
├── maze.rs
│   └── maker.rs
│       ├── algorithms.rs
│       └── algorithms/
│           ├── backtrack.rs
│           ├── prim.rs
│           └── wilson.rs
├── net.rs
├── player.rs
├── protocol.rs
├── ring.rs
├── snapshot.rs
└── time.rs
```