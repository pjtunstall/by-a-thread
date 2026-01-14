# By a Thread

## Context

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). I used Macroquad, a simple game library, to render scenes, the Renet crate for some networking abstractions over UDP.

## Netcode

## Renet

Renet defines three channel types: `ReliableOrdered`, `ReliableUnordered`, and `Unreliable`. I've used `ReliableOrdered` for system and chat messages and for bullet spawn and collision notifications from the server. I used `Unreliable` for input from the client, and for snapshots (player position updates) from the server.

## Buffers and history

The client maintains ring buffers called `input_history` (for their own inputs) and `snapshot_buffer` (for player-state updates from the server: position and, in the case of the local player, also velocity). The server maintains an `input_buffer` ring buffer for each player to store their inputs till it's time to process them.

The client forward-date its inputs to a time when we can fairly safely expect the server to have received them, hence the need for input buffers to store them serverside; see [Input](#input). Because they're sent on an unreliable channel (for speed), the client sends its last four inputs for redundancy, in case some fail to arrive, hence the need for an input history. The input history is also used to apply past inputs to snapshots; see [Local player](#local-player-reconciliation-replay-and-prediction).

The `input_history` is implemented as a `Ring` struct, and the others with the `NetworkBuffer` struct. A `Ring` stores items in an array, labeled with a 64-bit tick number. The index at which an item is inserted is its tick modulo the length of the array. This allows items to be inserted in a circular fashion. Since they're labeled with the tick number, the item corresponding to a given tick can be extracted; if the item at the corresponding index doesn't match the tick, the item for that tick is considered not found.

A `NetworkBuffer` includes a `Ring` together with a `head` and `tail`. The `head` is a "write" cursor. It's the tick of most recent item inserted. The `tail` is a "read" cursor. It's the tick of the last input processed, and is kept a a safety margin of ticks (a minute's worth) behind the last snapshot used. (The reason for this generous safety margin is that the client's estimate of current server time is not monotonic: it can slip backwards slightly due to network conditions.)

To save on bandwidth, ticks are sent as 16-bit unsigned integers and expanded into 64-bit tick numbers, based on the assumption that they're close to `head`.

### Input

Clients stamp their inputs with the tick number on which the server should process them: a few ticks in the future to account for latency, plus a small safety margin for network jitter. Inputs are sent on an `Unreliable` Renet channel. The client sends inputs for the last four ticks as redundancy, to reduce the chances that the server will be missing inputs due to lost packets.

### Local player: reconciliation, replay, and prediction

Local player means the player as represented on their own machine. First we reconcile to the last snapshot. Then we run client‑side prediction. This consists of replaying inputs from `input_history` up to the last simulated tick. Finally, we run the simulation further for as many ticks as needed to account for the duration of the last frame. The simulation includes checking for new inputs and applying them to the local player's state. It also inlcudes bullet updates; see [Bullets](#bullets-extrapolation).

### Remote players: interpolation

Remote players are the other players as represented on a given player's machine. Remote players are shown as they were 100ms in the past. To accomplish this, we find the latest snapshot from before this time and the earliest snapshot after it, and interpolate the remote players between where they were at those ticks.

### Bullets: extrapolation, AKA dead reckoning

These are actually glowing spheres that bounce off walls and floor, and players while their health lasts.

When the local player fires, a provisional bullet is spawned. Details are sent to the server along with an id. When the server confirms that the bullet was fired, this id is used to "promote" the provisional bullet. The client extrapolates the position of the confirmed bullet at the last simulated tick. That position is advanced by the simulation each tick. Over the next few ticks, the bullet's displayed position is blended towards the actual position, as advanced from the extrapolation.

Similarly, when the client receives details of a bullet fired by a remote player, the bullet's actual position is extrapolated to the last simulated tick and advanced from there each tick. The displayed bullet is first placed at the shooter's interpolated position, then blended (fast-forwarded), over the next few ticks, towards its actual position.

The client simulates bounces, but the server sends authoritative notification of all collisions, and the client then snaps the bullet to its new position.

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
