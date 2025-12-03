# Netcode plan

## Preliminaries

Choose a tick frequency, 60Hz (once every 16.7ms), and a broadcast frequency, e.g. 20Hz (once every 50.0ms). Decide how many client inputs to send to the server per tick for redundancy, e.g. 4.

Give the server a `Vec<ServerPlayer>` to store the player data. `ServerPlayer` contains a `Player`. Perhaps store it in the server's `Game` state.

## Terminology

NOTE: Client tick, server tick, and frame are conceptually distinct, but happen to have the same duration in this case. All three loops run at 60Hz. On the other hand, the server will broadcast at only 20Hz to reduce bandwidth.

- baseline: the most recent **snapshot** a client has received from the server.
- frame: one iteration of the client render loop, i.e. one instance of painting a scene. See also **tick**.
- `input_buffer: Vec<PlayerInput>`, capacity 128: a record the server uses to store inputs as they're received from a player. The server has one for each player and inserts newly received inputs at index `tick % 128`.
- `input_history: Vec<PlayerInput>`, capacity 256: a record the client uses to store their own inputs to be replayed on top of the current baseline state, i.e. the latest snapshot received from the server, in a process known as reconciliation and prediction.
- `JITTER_SAFETY_MARGIN: f64`: a safety margin of 50ms (about 3 ticks) to give player inputs more chance to arrive at the server in case of occasional delays.
- prediction: a process whereby the client replays inputs from its `input_history` for the ticks from immediately after its **baseline** state up to (and including) its most recent input.
- reconciliation: a process whereby the client sets the current state of its physics simulation to the latest **snapshot** (state received from the server); see also **baseline**. The client immediately replays its inputs for subsequent ticks on top of this till it reaches its own current tick, a process known as **prediction**.
- snapshot: the complete game state on a given tick, as calculated by the server, and broadcast to clients. See also **baseline**.
  `snapshot_buffer: Vec<Snapshot>`, capacity 8: (also known as an interpolation buffer) a record the client keeps of snapshots received so that it can interpolate
- tick: one iteration of the (client or server) physics simulation. Compare **frame**.

Regarding the lengths of the `Vecs`:

- 128 ticks -> ~2.1s.
- 256 ticks -> ~4.3s.
- 8 broadcasts -> 0.4s.

Check Renet config to see how long it takes for clients to actually time out.

## Server

### Players

Initialize an array as an input buffer for each player. The size should be a power of 2 that is larger than the maximum expected latency window, e.g., 128 (2s at 60Hz).

We'll receive inputs from players as a sequence of `PlayerInput`s. Several inputs are sent per message for the sake of redundancy: to reduce the risk of missing inputs. Each `PlayerInput` will include a tick id number (`u64`). The tick id number with the input is not that of the tick on which the client sent it; rather it's the client's request for which tick it wants the server to processes it. The client calculates this number based on smoothed rtt and a safety margin. The goal is to ensure that inputs from all clients are processed a similar amount of time after they were sent, for consistency and so as not to give any one player and unfair advantage under normal conditions.

Insert these inputs into the relevant player's input buffer at index `tick % INPUT_BUFFER_LENGTH`. Each tick, update the physics simulation, using the relevant input for each player, if available. The client always sends a `PlayerInput`, even if it's just to say there's no input. If no `PlayerInput` has been received yet from some client for the tick being processed, use the most recent earlier input received from that client.

Be sure to check that the tick id at the relevant index is correct in case no input for that client has been received yet and the array contains old data at that index. This is necessary in any case, but, as an added protection, replace processed inputs with `None` and store the last processed input in a separate variable in case we need to use it later as a guess for a missing value.

Safety cap: if no new input has arrived in the last 0.5s (30 ticks), then set the most recent to `None` to prevent a disconnected player from moving indefinitely.

At the broadcast frequency, broadcast the resulting game state, including positions of all players and bullets, and orientations of players, to all clients on an `Unreliable` Renet channel, tagged with the current tick number. More seriously consequential game events--in this case, just player death--are sent on a `ReliableOrdered` Renet channel. Everything else can go on the `Unreliable` channel. Even nonlethal hits can go on the `Unreliable` state channel; the health bar will adjust to the correct value when the update comes.

NOTE: Send current health rather than "player took X amount of damge". And, in general, always sync the value not the delta on an `Unreliable` channel; the same goes for position, orientation, ammo, etc.

## Client

### Local player: reconciliation and prediction

Each iteration of its game loop, update the client's [estimate of the server clock](../client/src/time.rs) and use it to calculate a server tick number, i.e. the tick on which it intends the server to process its (the client's) current inputs.

Check for inputs and insert them as a `PlayerInput` (containing all current keypresses along with the server tick number) to an array, `history_buffer`, of size 256. Check for messages from the server. If the server has sent an authoritative snapshot of the game state, set this as the new baseline by updating a variable that will track the index of the most recent baseline. Either way, reconcile the client's game state to that of the baseline, then--before rendering anything, purely in the client's physics simulation--replay its inputs for subsequent ticks from the baseline to the most recent input ('prediction'). Finally, renders the result, smoothing the transition from current position to the new estimate.

Check for new inputs and send the most recent handful of inputs to the server on an `Unreliable` Renet channel. This redundancy increases the chance that the server will have inputs available for each tick it processes and not have to guess.

NOTE: To be consistent with the server, ensure the physics update uses a fixed timestep (e.g., 1.0/60.0) and not `macroquad::time::get_frame_time()`.

Here is more detail on the client's time coordination logic, using the idea of a dynamic lead. Combining NTP (Network Time Protocol), defined by the clock estimate, with a clamped nudge, turns the game loop into what's known as a Control System (specifically a Proportional Controller with Saturation). We combine NTP with with dynamic nudge and spike handling.

```rust
// --- CONSTANTS ---
const SERVER_TICK_RATE: f64 = 60.0;
const TICK_DURATION_IDEAL: f64 = 1.0 / SERVER_TICK_RATE;
// Three ticks (50ms) is probably a safe starting buffer.
// If inputs arrive late on the server, increase this.
const JITTER_SAFETY_MARGIN: f64 = 0.05; // Consider raising to 4 ticks?

// --- THE LOOP ---

// 1. MEASURE REAL TIME
let raw_delta_time = macroquad::time::get_frame_time(); // Consider using std::time.
let tick_duration_actual = std::time::Duration::from_f64(raw_delta_time);

// 2. UPDATE BASELINES
// A. Update the "radar" (client's estimate of current server time).
// This keeps session.estimated_server_time aligned with the server's clock.
update_clock(&mut session, &mut network, tick_duration_actual);

// B. Update the "road conditions" (RTT).
// We use asymmetric smoothing:
// - If RTT goes UP (lag spike), we adapt QUICKLY (0.1) to prevent input starvation.
// - If RTT goes DOWN (improvement), we adapt SLOWLY (0.01) to keep simulation stable.
let current_rtt = network.rtt().clamp(0.0, 1.0); // Discard excessively long rtt.
let rtt_alpha = if current_rtt > session.smoothed_rtt { 0.1 } else { 0.01 };

// Simple linear interpolation.
// Encapsulate as `lerp(session.smoothed_rtt, renet.rtt(), alpha)`.
session.smoothed_rtt = session.smoothed_rtt * (1.0 - rtt_alpha) + current_rtt * rtt_alpha;

// 3. CALCULATE TARGET TIME
// Target = "What time is it now" + "Travel Time" + "Safety Margin".
let travel_time = session.smoothed_rtt / 2.0;
let target_sim_time = session.estimated_server_time + travel_time + JITTER_SAFETY_MARGIN;

// And, from that, calculate the target tick and pass it along with the current_tick to process_input?

// 4. CALCULATE ERROR
// "Where we should be" minus "Where we are".
let error = target_sim_time - session.simulated_time;

// 5. THE HYBRID CONTROL SYSTEM
let adjustment = if error.abs() > 0.25 {
    // CASE A: HARD SNAP
    // We are > 250ms off. The internet choked or we just connected.
    // Teleport immediately to avoid speeding up for 10 seconds.
    println!("Resyncing clock... Delta: {:.4}s", error);

    // We force the error to be exactly enough to close the gap instantly.
    error
} else {
    // CASE B: CLAMPED NUDGE
    // We are slightly off. Nudge the clock by +/- 10% of the error.
    // Limit the nudge to +/- 2ms per frame to prevent visual stutter.
    (error * 0.1).clamp(-0.002, 0.002)
};

// 6. FILL ACCUMULATOR
// We add Real Time + The Adjustment.
// If we are behind, adjustment is positive (simulation runs faster).
// If we are ahead, adjustment is negative (simulation runs slower).
session.accumulator += raw_delta_time + adjustment;

// 7. PHYSICS LOOP (FIXED STEP)
const MAX_TICKS_PER_FRAME: u32 = 8; // A failsafe to prevent the accumulator from growing
let mut ticks_processed = 0;        // ever greater if we fall behind.
while session.accumulator >= TICK_DURATION_IDEAL && ticks_processed < MAX_TICKS_PER_FRAME  {
    process_input(session.current_tick); // Insert into history, send to server.
    perform_tick(session.current_tick); // Run physics: reconcile and predict.

    // C. Advance State.
    session.accumulator -= TICK_DURATION_IDEAL;
    session.current_tick += 1;
    ticks_processed += 1;

    // Track our time using the fixed step to stay perfectly in sync with ticks.
    session.simulated_time += TICK_DURATION_IDEAL;

    // If we hit the limit, discard the remaining accumulator to prevent spiral.
    if ticks_processed >= MAX_TICKS_PER_FRAME {
        session.accumulator = 0.0; // Or keep a small remainder, but discard the bulk.
        println!("Physics spiral detected: skipped ticks to catch up.");
    }
}

// 8. RENDER INTERPOLATION
let alpha = session.accumulator / TICK_DURATION_IDEAL;
render(alpha);

macroquad::window::next_frame().await;
```

Check for reliable messages before unreliable. That will allow us to set local player status to dead before attempting reconciliation or other logic.

```rust
async fn client_tick() {
    // 1. UPDATE TIME (NTP-style)
    update_clock();

    // 2. HANDLE CRITICAL EVENTS (before simulation).
    process_reliable_messages();  // Only deaths.

    // 3. RECONCILE WITH SERVER (only if alive).
    if session.is_alive {
        if let Some(snapshot) = get_latest_snapshot() {
            reconcile_and_replay(snapshot);
        }
    }

    // 4. PROCESS NEW INPUT (only if alive).
    if session.is_alive {
        process_current_input();
        send_inputs_to_server();
    } else {
        // Handle death state.
        handle_death_state();
    }

    // 5. RENDER
    render();
}
```

### Remote players: interpolation, rather than dead reckoning (extrapolation)

Insert snapshots into a snapshot buffer (1s+). Where does that "1s+" come from? is that another opinion on how much time it should cover?

```rust
const INTERPOLATION_DELAY = 0.1; // 100ms.

let render_time = session.estimated_server_time - INTERPOLATION_DELAY;
let render_tick_f64 = (render_time / SERVER_TICK_DURATION).floor();
let render_tick = render_tick_f64 as u64;
```

Do we have the snapshot for `render_tick` and the tick after? Then render the state with all values at `t` times the difference between the value as it was at the `render_tick` and how it was at the next tick, where `t` is the fractional part of `render_time`, i.e. `render_time - render_tick_f64` (the difference between `render_time` and the time of the `render_tick`).

Q. What to do if suitable snapshots aren't available? Render between further apart ones? What if no later snapshot is available, or no earlier one? What is the most likely way that things can go wrong and how to handle it? What to do on startup: wait for snapshots and skip rendering other players? It's likely to be momentary.

## FAQ

Q: Why do we interpolate?
A: To mitigate network jitter (smooth it out, preventing small fluctuations from causing a whole earlier or later server tick to be rendered) and low broadcast rate (fill in the gaps between broadcast snapshots).

Q. Why do we show snapshots at a bigger delay than we need to, e.g. 100ms?
A. To give snapshots more chance to arrive, analogous to how the server maintains an input buffer.

Q: Why have a low broadcast rate? That is, why have the server update its physics simulation at a higher frequency than it broadcasts snapshots?
A: The slower broadcast rate saves on bandwidth. The faster physics rate prevents tunneling/teleportation. If items moved at the broadcast rate, they'd be more likely to teleport through obstacles.

## Implementation

Consider wraparound ticks of some smaller data type than `u64`.

```rust
pub trait Sequenced {
    fn sequence(&self) -> u16;
}

#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buffer: Vec<T>,
    mask: usize,
}

impl<T> RingBuffer<T>
where T: Clone + Sequenced + Default {
    pub fn new(size: usize) -> Self {
        assert!(size > 0 && (size & (size - 1)) == 0, "Size must be power of 2");

        // Create a dummy element with a sequence number that won't collide
        // with the first real packet.
        // Assuming the game starts at tick 0 or 1, u16::MAX is a safe sentinel.
        let mut dummy = T::default();

        // we might need a setter for this, or just ensure Default returns
        // a sequence (like 0) and handle the game start logic to avoid collision.
        // Ideally: dummy.set_sequence(u16::MAX);

        Self {
            buffer: vec![dummy; size],
            mask: size - 1,
        }
    }

    pub fn insert(&mut self, item: T) {
        let seq = item.sequence();
        let index = seq as usize & self.mask;

        self.buffer[index] = item;
    }

    pub fn get(&self, sequence: u16) -> Option<&T> {
        let index = sequence as usize & self.mask;
        let item = &self.buffer[index];

        if item.sequence() == sequence {
            Some(item)
        } else {
            None
        }
    }

    pub fn get_raw(&self, sequence: u16) -> &T {
        let index = sequence as usize & self.mask;
        &self.buffer[index]
    }
}
```

Or, rawer:

```rust
pub const INPUT_BUFFER_SIZE: usize = 128;
pub const INPUT_HISTORY_SIZE: usize = 256;
pub const SNAPSHOT_BUFFER_SIZE: usize = 8;

assert!(INPUT_BUFFER_SIZE != 0, "INPUT_BUFFER_SIZE should not be 0");
assert!(INPUT_BUFFER_SIZE & (INPUT_BUFFER_SIZE - 1) == 0, "INPUT_BUFFER_SIZE should be a power of 2");

assert!(INPUT_HISTORY_SIZE != 0, "INPUT_HISTORY_SIZE should not be 0");
assert!(INPUT_HISTORY_SIZE & (INPUT_HISTORY_SIZE - 1) == 0, "INPUT_HISTORY_SIZE should be a power of 2");

assert!(SNAPSHOT_BUFFER_SIZE != 0, "SNAPSHOT_BUFFER_SIZE should not be 0");
assert!(SNAPSHOT_BUFFER_SIZE & (SNAPSHOT_BUFFER_SIZE - 1) == 0, "SNAPSHOT_BUFFER_SIZE should be a power of 2");
```

And everywhere that items are accessed:

```rust
let input = &buffer[target_tick];

if input.tick == target_tick {
    // Valid data.
} else {
    // Packet loss (or future data). Handle accordingly.
}
```

Arrays for inputs and a Vec for the snapshot buffer? First consider what a snapshot will actually consist of. Include only bullet spawn, bounce, and expiry events? Reconsider struct of arrays.

Boxed array recommended for snapshot buffer. Safest to create it on heap to begin with via a Vec, to avoid the risk of stack overflow on initialization, because the array is initialized on the stack then moved to the heap as it's boxed.

```rust
pub fn new_boxed_buffer<T: Default, const N: usize>() -> Box<[T; N]> {
    // 1. Create a Vec (allocates directly on heap).
    let v: Vec<T> = (0..N).map(|_| T::default()).collect();

    // 2. Convert to Box<[T]> (drops capacity field).
    let slice = v.into_boxed_slice();

    // 3. Convert slice to Array (guaranteed to succeed if size matches).
    // unwrapping is safe here because we controlled the size above.
    slice.try_into().expect("length mismatch in array creation")
}
```

Distinguish between memory layout and wire format. Configure serde to only send what's needed:

```rust
use serde::{Serialize, Serializer, Deserialize, Deserializer, ser::SerializeStruct};
use serde::ser::SerializeStruct;
use serde::de::{self, SeqAccess, Visitor};
use std::fmt;

const MAX_PLAYERS: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub tick: u16,
    pub active_mask: u16,
    pub players: [PlayerState; MAX_PLAYERS],
}

struct PlayerState {
    pos_x: f32, // 4 bytes (Primitive)
    pos_y: f32, // 4 bytes (Primitive)
    pos_z: f32, // 4 bytes (Primitive)
    rot_w: f32, // 4 bytes (Primitive)
    // ... etc
}

// 1. The Helper Wrapper
// It holds a reference to the Snapshot, so it costs nothing to create.
struct ActivePlayersIter<'a>(&'a Snapshot);

impl<'a> Serialize for ActivePlayersIter<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        // 2. The Logic
        // Create an iterator that filters based on the bitmask.
        let iter = self.0.players.iter()
            .enumerate()
            .filter_map(|(i, player)| {
                if (self.0.active_mask & (1 << i)) != 0 {
                    Some(player)
                } else {
                    None
                }
            });

        // 3. The Magic
        // collect_seq consumes the iterator and writes directly to the wire.
        // No Vec. No Heap.
        serializer.collect_seq(iter)
    }
}

// -----------------------------------------------------------

impl Serialize for Snapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut state = serializer.serialize_struct("Snapshot", 3)?;

        state.serialize_field("tick", &self.tick)?;
        state.serialize_field("active_mask", &self.active_mask)?;

        // 4. Usage
        // We pass the wrapper. It doesn't allocate; it just passes the logic.
        state.serialize_field("players", &ActivePlayersIter(self))?;

        state.end()
    }
}

impl<'de> Visitor<'de> for SnapshotVisitor {
    type Value = Snapshot;

    fn visit_seq<V>(self, mut seq: V) -> Result<Snapshot, V::Error>
    where V: SeqAccess<'de> {
        // 1. Read metadata
        let tick = seq.next_element()?.ok_or_else(|| ...)?;
        let active_mask = seq.next_element()?.ok_or_else(|| ...)?;

        // 2. Prepare the destination (On the Stack / Inline)
        let mut players = [PlayerState::default(); MAX_PLAYERS];

        // 3. Iterate the mask to know where to put the incoming items
        for i in 0..MAX_PLAYERS {
            // Check if this slot expects data
            if (active_mask & (1 << i)) != 0 {
                // CRITICAL FIX:
                // We pull ONE item from the stream directly into the array slot.
                // No intermediate Vec. No Heap.
                players[i] = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
            }
        }

        Ok(Snapshot { tick, active_mask, players })
    }
}
```

## State vs. events

This is important! I need to understand what to do for the bullets before proceeding with the rest of the network logic. Each part informs the others: what data structures to use, what to send, and how.

Actually send bullet spawn, bounce, kill/player-death, and expiry events on the reliable channel. Bullet events will have bullet id, position, velocity, and time. Don't edit the snapshot buffer. The snapshot buffer (the ring buffer of arrays) is strictly for interpolating players. Bullets should live in a separate `Vec<Bullet>`. On receiving a bounce, say, the client updates the bullet's position to the bounce point, calculates the new velocity, and "simulates" 50ms worth of movement instantly to put the bullet where it should be right now. This is extrapolation, aka dead reckoning.

Feature.Transport.Storage Strategy................................Bandwidth Strategy
....................................................................................
players.unreliable.`Box<[Snapshot; 8]>` (snapshot ring buffer).Serialize only active players (slice)
bullets.reliable...`Vec<Bullet>` (world state entity list)......Send events, extrapolate locally
deaths..reliable...(event queue)................................Events

I can give the bullets Vec a capacity of 240.
