# State Machines

- [Overview](#overview)
- [Client State Machine](#client-state-machine)
  - [Lobby](#lobby)
- [Server State Machine](#server-state-machine)

# Overview

Both client and server use the state pattern to organize flow. Each has its own collection of states.

## Client State Machine

```txt
Lobby -> Game -> AfterGameChat
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state (or substate) can lead to `Disconnected`.

### Lobby

```
ServerAddress -> Passcode -> Connecting -> Authenticating -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat
```

`ServerAddress` prompts for an IP address and port number; pressing Enter uses the local default.

If the player is the host: `Chat -> ChoosingDifficulty`, otherwise `Chat -> Countdown`. In either case,

```txt
Countdown -> Game
```

## Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

The `Game` state also manages clients in `AfterGameChat` since they arrive at different times.