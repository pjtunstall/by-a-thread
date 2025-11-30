# Plan for Local Player Netcode

We choose a tick frequency, 60Hz.

## Server

The server pre-allocates a `VecDeque<Option<PlayerInput>>` as an input buffer for each player. The size should be a power of 2 that is larger than the maximum expected latency window (e.g., 128 or 256). When a message arrives, discard if its tick has already been processed, i.e. `if input_tick < server_current_tick`. Otherwise calculate its index as `index = tick % buffer_size` and insert it into the correct player's buffer.

The server receives inputs from players a `PlayerInput`, which includes player id and a tick number, which each client calculates, and which will correspond to the tick on which the server processes it. The stores them in the relevant player's input buffer. Each iteration of its game loop, i.e. each tick, the server processes inputs for each player, if available, for the server's own current tick. The client always sends a `PlayerInput`, even if that's just to say there's no input. If no `PlayerInput` has been received yet from some client for the tick being processed, the server guesses, using the most recent earlier input it received from that client. After processing the inputs, the server extracts and stores the input it has just processed from each player, so that it can use a client's input again on the next tick if no input for that next tick has been received from them. Finally, it prunes the `VecDeque` of any inputs from ticks earlier than the one that's just been processed.

The server broadcasts the resulting game state, including positions of all players and bullets, and orientations of players, to all clients on an `Unreliable` Renet channel, tagged with the number of the tick that was processed. More seriously consequential game events--in this case, just player death--are sent on a `ReliableOrdered` Renet channel. Everything else can go on the `Unreliable` channel. Even nonlethal hits can go on the `Unreliable` state channel; the health bar will just just to the correct value when the update comes. Note: send current health rather than "player took X amount of damge". In general, always sync the value not the delta on an `Unreliable` channel; the same goes for position, orientation, ammo, etc.

## Client

Each iteration of its game loop, the client updates its estimate of the server clock.

```rust
use std::time::Duration;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use shared::{self, net::AppChannel, protocol::ServerMessage};

pub fn update_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    interval: Duration,
) {
    // Increment by interval. This determines the simulation rate.
    session.estimated_server_time += interval.as_secs_f64();

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

It then uses that estimate to calculate a tick number.

It checks for inputs and pushes them as a `PlayerInput` (storing all current keypresses along with the tick number) to a `VecDeque` called `input_buffer`. It checks for messages from the server. The server should have sent an authoritative snapshot of the game state. This the client treats as its new baseline. First the client discards any inputs from ticks earlier than that of the baseline. Then sets its game state equal to the baseline ('reconciliation'), then--before rendering anything, purely in its physics simulation--replays its inputs for subsequent ticks from the baseline to the most recent input ('prediction'). Finally, it renders the result. (A refinement would be to smooth the transition from current position to the new estimate, but this is good enough for now.)

The client checks for new inputs and sends the most recent 4 inputs to the server on an `Unreliable` Renet channel. This redundancy increases the chance that the server will have inputs available for each tick it processes and not have to guess.

Note: To be consistent with the server, ensure the physics update uses a fixed timestep (e.g., 1.0/60.0) and not `macroquad::time::get_frame_time()`.

Here is more detail on the client's time coordination logic, using the idea of a dynamic lead. Combining NTP (Network Time Protocol), defined by the clock estimate, with a clamped nudge, turns the game loop into what's known as a Control System (specifically a Proportional Controller with Saturation).

We combine NTP with with dynamic nudge and spike handling:

```rust
// --- CONSTANTS ---
const SERVER_TICK_RATE: f64 = 60.0;
const IDEAL_TICK_DURATION: f64 = 1.0 / SERVER_TICK_RATE;
// Three ticks (50ms) is probably a safe starting buffer.
// If inputs arrive late on the server, increase this.
const JITTER_BUFFER_SECONDS: f64 = 0.050;

// --- THE LOOP ---

// 1. MEASURE REAL TIME
let raw_delta_time = macroquad::time::get_frame_time(); // Consider using std::time.
let actual_tick_duration = std::time::Duration::from_secs_f64(raw_delta_time);

// 2. UPDATE BASELINES
// A. Update the "radar" (client's estimate of current server time).
// This keeps session.estimated_server_time aligned with the server's clock.
update_clock(&mut session, &mut network, actual_tick_duration);

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
let target_sim_time = session.estimated_server_time + travel_time + JITTER_BUFFER_SECONDS;

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
while session.accumulator >= IDEAL_TICK_DURATION {
    // A. Process Inputs (Push to buffer, send to server).
    process_input(session.current_tick);

    // B. Run Physics Prediction.
    perform_tick(session.current_tick);

    // C. Advance State.
    session.accumulator -= IDEAL_TICK_DURATION;
    session.current_tick += 1;

    // Track our time using the fixed step to stay perfectly in sync with ticks.
    session.simulated_time += IDEAL_TICK_DURATION;
}

// 8. RENDER INTERPOLATION
let alpha = session.accumulator / IDEAL_TICK_DURATION;
render(alpha);

macroquad::window::next_frame().await;
```
