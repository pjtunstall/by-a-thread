# By a Thread

![screenshot](screenshot.jpg)

## Context

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973.

Dependencies: I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode.

## My game

It's not yet hosted, so actually playing with other players is not yet practical, but you can at least run the server and one or more clients on a single machine.

### To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --bin server` in one terminal. For each client, open another terminal and run `cargo run --bin client`. Then follow the prompts. The passcode will appear in the server terminal.

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.

### Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`.

## Netcode

Netcode refers to the techniques used to coordinate how players and other dynamic entities, such as projectiles, are displayed in a way that disguises latency. I found Gabriel Gambetta's introduction helpful: [Fast Paced Multiplayer](https://gabrielgambetta.com/client-server-game-architecture.html). Gemini was a great help too in developing a detailed plan.

In what follows, "local player" will mean a player as represented on their own machine. Remote players are the other players as represented on a given player's machine.

TT;DR: I used reconciliation and prediction for the local player, interpolation for remote players, and dead reckoning for bullets.

## Renet

Renet is a networking library for Rust, built on top of UDP. It defines three channel types: `ReliableOrdered`, `ReliableUnordered`, and `Unreliable`. You can send messages over these channels. Renet takes care of splitting them into packets and reassembling them. I've used `ReliableOrdered` for system and chat messages and for bullet spawn and collision notifications from the server. I used `Unreliable` for input from the client, and for snapshots (player position updates) from the server. `Unreliable` is faster because it doesn't have to order messages or resend ones that failed to arrive.

## Tick and frame

A frame is an iteration of the client's game loop. The frame rate is how often the latest game state is rendered on the screen, i.e. how fast the display is refreshed. On many computers this is ideally 60 frames per second; on some the ideal may be faster. If all work is done, the program waits for the rest of the ideal frame duration to have elapsed before continuing to the next iteration. If we ask the computer to do too much work, a frame could last longer. It can also last longer if we put the window into the background, in which case Macroquad detects that there's no point rendering and keeps waiting till the window is visible again.

A tick is an iteration of the server's game loop. But a tick is also a unit of game time: a game-logic update and, by extension, the sequence number that such an update is labeled by. The reason for this blurring of terminology is that the server is authoritative. Clients just have to trust that it keeps time well since they will try to synchronize their clocks with its, and that one tick lasts as long as it should and not longer. Luckily the server doesn't have to do any rendering, so it's less likely to be overwhelmed.

Although the server runs its input processing and physics updates at 60Hz, it only broadcasts player positions at 20Hz. I've called this the broadcast rate. The client has various ways of filling in the gaps, as detailed below.

Depending on varying latency and frame duration, the client may have a varying number of ticks (in the sense of game-logic updates) to process each frame. Even if it hasn't heard from the server on a given tick, it must still check its own inputs and update its "simulation" of the server' sauthoritative reality. When data does arrive, it will correct the simulation, although it may do so in subtle ways, smoothing out abrupt changes.

## Clock synchronization

The client needs a good estimate of server time to drive interpolation, input scheduling, and countdowns, but it can't trust wall clocks: packets arrive late, late packets can arrive out of order, and RTT (return travel time) jitters with network conditions. So the client builds a moving estimate, `estimated_server_time`, from periodic server pings and smooths it to avoid visible stutter.

The server broadcasts `ServerTime` messages at a fixed interval. The client records each message as a `ClockSample` with the server time, the local receive time (a monotonic clock), and the RTT from Renet. Each frame, the estimate is advanced by the duration of the last frame. When samples are available, the client chooses the best one by minimizing `rtt + age * AGE_PENALTY_FACTOR`, so it prefers a slightly higher RTT from a fresh packet over a perfect RTT from an old packet. It then computes a target time as `server_time + rtt / 2 + age_of_sample`.

If this is the first estimate or the error exceeds one second, the clock snaps to the target. Otherwise, small errors inside a deadzone are ignored, and larger errors are nudged toward the target using a small smoothing factor (speeding up or slowing down symmetrically). RTT itself is smoothed with different alphas for spikes vs. improvements, and the smoothed RTT feeds later timing decisions like the simulation target time.

## Buffers and history

The client maintains ring buffers called `input_history` (for their own inputs, 256 ticks, ~4.3s at 60Hz) and `snapshot_buffer` (for player position updates, 16 broadcasts, ~0.8s at 20Hz). The server maintains an `input_buffer` ring buffer for each player to store their inputs till it's time to process them (128 ticks, ~2.1s at 60Hz).

The `input_history` is implemented as a `Ring` struct, and the others with the `NetworkBuffer` struct. A `Ring` stores items in an array, labeled with a 64-bit tick number. The index at which an item is inserted is its tick modulo the length of the array. This allows items to be inserted in a circular fashion. Since they're labeled with the tick number, the item corresponding to a given tick can be extracted; if the item at the corresponding index doesn't match the tick, the item for that tick is considered not found.

A `NetworkBuffer` includes a `Ring` together with a `head` and `tail`. The `head` is a "write" cursor. It's the tick of most recent item inserted. The `tail` is a "read" cursor. It's the tick of the last input processed, and is kept a a safety margin of ticks (a minute's worth) behind the last snapshot used. (The reason for this generous safety margin is that the client's estimate of current server time is not monotonic: it can slip backwards slightly due to network conditions.)

To save on bandwidth, ticks are sent as 16-bit unsigned integers and expanded into 64-bit tick numbers, based on the assumption that they're close to `head`.

### Input

The client forward-dates its inputs to give them time to reach the server. They're actually forward-dated a little bit more than necessary as a safety margin, hence the need for input buffers to store them serverside. The client sends its last four inputs each tick, in case some fail to arrive on the `Unreliable` channel, hence the need for an input history. The input history is also used to apply past inputs to snapshots; see [Local player](#local-player-reconciliation-replay-and-prediction).

Players send input data for every tick, even if it's just to say that no keys are pressed.

As the server performs its game-state update (processes a tick), it checks each player's `input_buffer` to see if there's an input available for this tick. If so, it applies that input. If not, it re-applies the last input that it did receive.

### Local player: reconciliation and prediction

First we reconcile to the last snapshot. Then we run client‑side prediction. This consists of replaying inputs from `input_history` up to the last simulated tick. Finally, we run the simulation further for as many ticks as needed to account for the duration of the last frame. The simulation includes checking for new inputs and applying them to the local player's state. It also inlcudes bullet updates; see [Bullets](#bullets-extrapolation).

Snapshots inlcude everyone's position. Also included is the recipient's velocity. This allows them to fully simulate their own state during prediction.

### Remote players: interpolation

Remote players are shown as they were 100ms in the past. Of course, we can only know where they were in the past, but we actually place them a little bit further in the past to ensure smoothness rather than letting rapidly changing data from snapshots battle with our latest estimation. To accomplish this trick, we find the closest snapshots on either side of this time (the latest snapshot from before it and the earliest snapshot after it), and interpolate the remote players between where they were at those ticks.

### Bullets: extrapolation, AKA dead reckoning

These are actually plasma spheres that bounce off walls and floor. They also bounce off players while their health lasts.

When the local player fires, a provisional bullet is spawned. Details are sent to the server along with an id. When the server confirms that the bullet was fired, this id is used to "promote" the provisional bullet. The client extrapolates the position of the confirmed bullet at the last simulated tick. That position is advanced by the simulation each tick. Over the next few ticks, the bullet's displayed position is blended towards the actual position, as advanced from the extrapolation.

Similarly, when the client receives details of a bullet fired by a remote player, the bullet's actual position is extrapolated to the last simulated tick and advanced from there each tick. The displayed bullet is first placed at the shooter's interpolated position, then blended (fast-forwarded), over the next few ticks, towards its actual position.

The client simulates bounces, but the server sends authoritative notification of all collisions, and the client then snaps the bullet to its new position.

Some games use an alternative technique known as lag compensation. In that more Orwellian approach, the shooter is favored. The server calculates where they saw the target at the time of shooting, and makes that its authoritative truth. Maybe you know a game like this. Lag compensation is best suited to games with extremely fast projectiles. If the target has no time to dodge, they often can't be sure that they weren't in their enemies sights.

Conversely, in games with projectiles that are slow enough that you can see them coming, you might feel cheated because you knew you dodged the bullet, while the shooter is likely to be less sure that they compensated correctly for a moving target. For this reason, I chose not to implement lag compensation here.

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

## Workflow

I used a variety of AI chatbots to straighten out my understanding of the netcode and develop a strategy, to help troubleshoot, and to review my attempts. Gemini (Pro) is the one that really shone this time, especially after the launch of Gemini 3. Towards the end, I enjoyed a trial of Codex. It was useful for discussing ideas, and a fantastic time-saver when it came to tidying up the loose ends.

## Credits

### Sound effects

Sound effects from Yodguard and freesound_community via Pixabay.

### Images

[Griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) fresco adapted from a photo by Vadim Indeikin.

[Bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) frescos, Gleb Simonov.

[Blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091) by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
