# By a Thread

## Context

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883).

## Netcode

The client maintains an `input_history` ring buffer and a `snapshot_buffer` for player state updates from the server. The server maintains an `input_buffer` ring buffer for each player to store their inputs till it's time to process them. Clients stamp their inputs with the tick number on which the server should process them: a few ticks in the future to account for latency, plus a small safety margin for network jitter. Inputs are sent on an `Unreliable` Renet channel. The client sends inputs for the last four ticks as redundancy, to reduce the chances that the server will be missing inputs due to lost packets.

### Local player: reconciliation, replay, and prediction

First we reconcile to the last snapshot. Then we run client‑side prediction. This consists of replaying inputs from `input_history` up to the last simulated tick. Finally, we run the simulation further for as many ticks as needed to account for the duration of the last frame. The simulation includes checking for new inputs and applying them to the local player's state. It also inlcudes bullet updates; see [below](#bullets-extrapolation).

### Remote players: interpolation

### Bullets: extrapolation

When the local player fires, a provisional bullet is spawned. Details are sent to the server along with an id. When the server confirms that the bullet was fired, this id is used to "promote" the provisional bullet. The client extrapolates the position of the confirmed bullet at the last simulated tick. That position is advanced by the simulation each tick. Over the next few ticks, the bullet's displayed position is blended towards the actual position, as advanced from the extrapolation.

Similarly, when the client receives details of a bullet fired by a remote player, the bullet's actual position is extrapolated to the last simulated tick and advanced from there each tick. The displayed bullet is first placed at the shooter's interpolated position, then blended (fast-forwarded), over the next few ticks, towards its actual position.

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
