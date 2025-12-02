# Netcode plan

## Preliminaries

Choose a tick frequency, 60Hz (once every 16.7ms), and a broadcast frequency, e.g. 20Hz (once every 50.0ms). Decide how many client inputs to send to the server per tick for redundancy, e.g. 4.

Give the server a `Vec<ServerPlayer>` to store the player data. `ServerPlayer` contains a `Player`. Perhaps store it in the server's `Game` state.

## Terminology

NOTE: Client tick, server tick, and frame are conceptually distinct, but happen to have the same duration in this case. All three loops run at 60Hz. On the other hand, the server will broadcast at only 20Hz to reduce bandwidth.

- baseline: the most recent **snapshot** a client has received from the server.
- frame: one iteration of the client render loop, i.e. one instance of painting a scene. See also **tick**.
- `input_buffer: [Option<PlayerInput>: 128]`: an array the server uses to store inputs as they're received from a player. The server has one for each player and inserts newly received inputs at index `tick % 128`.
- `input_history: [Option<PlayerInput>: 256]`: an array the client uses to store their own inputs to be replayed on top of the current baseline state, i.e. the latest snapshot received from the server, in a process known as reconciliation and prediction.
- `JITTER_SAFETY_MARGIN: f64`: a safety margin of 50ms (about 3 ticks) to give player inputs more chance to arrive at the server in case of occasional delays.
- prediction: a process whereby the client replays inputs from its `input_history` for the ticks from immediately after its **baseline** state up to (and including) its most recent input.
- reconciliation: a process whereby the client sets the current state of its physics simulation to the latest **snapshot** (state received from the server); see also **baseline**. The client immediately replays its inputs for subsequent ticks on top of this till it reaches its own current tick, a process known as **prediction**.
- snapshot: the complete game state on a given tick, as calculated by the server, and broadcast to clients. See also **baseline**.
- tick: one iteration of the (client or server) physics simulation. Compare **frame**.

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

### Local player: reconcilile and predict

TODO: Extract magic numbers (0.1, 0.3, 0.5, 0.002) into named constants.

Each iteration of its game loop, update the client's estimate of the server clock, thus:

```rust
use std::time::Duration;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use common::{self, net::AppChannel, protocol::ServerMessage};

pub fn update_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    interval: Duration,
) {
    // Increment by interval. This determines the simulation rate.
    session.estimated_server_time += interval.as_f64();

    // Get time messages that the server sent for us to sync our clock.
    // Only process the most recent message, discard older queued ones.
    let mut latest_message = None;
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        latest_message = Some(message);
    }

    let Some(message) = latest_message else {
        return;
    };

    match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
        Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
            let rtt = network.rtt();

            // Reject obviously bad samples (e.g., network spikes).
            const MAX_REASONABLE_RTT: f64 = 1.0; // 1 second.
            if rtt > MAX_REASONABLE_RTT {
                println!("Excessive rtt observed while updating clock: {}", rtt);
                return;
            }

            let one_way_latency = rtt / 2.0;
            let target_time = server_sent_time + one_way_latency;
            let delta = target_time - session.estimated_server_time;

            // Large delta, more smoothing (likely clock jump or initial sync).
            // Small deltas, less smoothing (normal jitter correction).
            let alpha = if delta.abs() > 0.1 {
                0.3 // Smooth large corrections over ~3 updates.
            } else {
                0.5 // Apply small corrections more quickly.
            };

            session.estimated_server_time += delta * alpha;
        }
        Err(e) => {
            eprintln!("Failed to deserialize ServerTime message: {}", e);
        }
        _ => {}
    }
}

```

Use that estimate to calculate a server tick number.

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
const JITTER_SAFETY_MARGIN: f64 = 0.050;

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
let current_rtt = network.rtt().clamp(0.0, 1.0); // Discard excessively long rtt. (Log this!)
let rtt_alpha = if current_rtt > session.smoothed_rtt { 0.1 } else { 0.01 };

// Simple linear interpolation.
// Encapsulate as `lerp(session.smoothed_rtt, renet.rtt(), alpha)`.
session.smoothed_rtt = session.smoothed_rtt * (1.0 - rtt_alpha) + current_rtt * rtt_alpha;

// 3. CALCULATE TARGET TIME
// Target = "What time is it now" + "Travel Time" + "Safety Margin".
let travel_time = session.smoothed_rtt / 2.0;
let target_sim_time = session.estimated_server_time + travel_time + JITTER_SAFETY_MARGIN;

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

### Remote players: interpolate

Q: Why do we interpolate?
A: To mitigate network jitter and low broadcast rate.

Q: Why have a low broadcast rate? That is, why have the server update its physics simulation at a higher frequency than it broadcasts snapshots?
A: The slower broadcast rate saves on bandwidth. The faster physics rate prevents tunneling/teleportation. If items moved at the broadcast rate, they'd be more likely to teleport through obstacles.
