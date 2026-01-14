# By a Thread

## Context

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883).

## Netcode

The client maintains an `input_history` ring buffer and a `snapshot_buffer` for player state updates from the server. The server maintains an `input_buffer` ring buffer for each player to store their inputs till it's time to process them.

### Local player: reconciliation, replay, and prediction

First we reconcile to the last snapshot. Then we run client‑side prediction. This consists of replaying inputs from `input_history` up to the last simulated tick. Finally, we run the simulation further for as many ticks as needed to account for the duration of the last frame. The simulation includes checking for new inputs and applying them to the local player's state. It also inlcudes bullet updates; see [below](#bullets-extrapolation).

### Remote players: interpolation

### Bullets: extrapolation

## State Machines

### Client State Machine

```txt
Lobby -> Game -> AfterGameChat
```

Lobby has various substates, as detailed [below](#lobby).

From the Lobby substate `Connecting` onwards, any state (or substate) can lead to `Disconnected`.

#### Lobby

```
Startup -> Connecting -> Authenticating -> ChoosingUsername <-> AwaitingUsernameConfirmation -> Chat
```

If the player is the host: `Chat -> ChoosingDifficulty`, otherwise `Chat -> Countdown`. In either case,

```txt
Countdown -> Game
```

### Server State Machine

```
Lobby -> ChoosingDifficulty -> Countdown -> Game
```

The `Game` state also manages clients in `AfterGameChat` since they arrive at different times.
