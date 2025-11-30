# Plan for Local Player Netcode

We choose a tick frequency, 60Hz. We choose a lead, for now just a fixed number of ticks, erring on the side of slightly larger than we're likely to need: 4. The lead represents how far in the past the server will treat as the 'current' tick to be processed. It's necessary to account for the time it takes to receive inputs from the player. We want to make sure there's plenty of time for inputs to reach the server.

REVISION: 4 ticks at 60Hz is 66ms, which might be too small. Instead, use a dynamic lead: `rtt / 2 + jitter_buffer`. Every frame,

```rust
smoothed_rtt = lerp(smoothed_rtt, renet.rtt(), 0.01);
target = smoothed_rtt / tick_duration + 3;
```

Is this smoothing superfluous, now, given that we're using the estimate of server time that incorporates its own smoothing?

## Server

The server receives inputs from players a `PlayerInput`, which includes player id and a tick number. It stores them in a `BTreeMap`, i.e. an ordered hash map, ordered by tick. (REVISION: Use a `VecDeque` instead of a `BTreeMap`. Better performance.) Each iteration of its game loop, i.e. each tick, it processes inputs for each player from 4 ticks earlier than the server's own current tick, if available. The client always sends a `PlayerInput`, even if that's just to say there's no input. If no `PlayerInput` has been received yet from some client for the tick being processed, the server guesses, using the most recent earlier input it received from that client. After processing the inputs, the server prunes the map of any inputs from ticks earlier than the 'current' one, i.e. the tick number that's being processed, after extracting and storing the input it has just processed from each player, so that it can use a client's input again on the next tick if no input for that next tick has been received from them.

The server broadcasts the resulting game state, inlcuding positions of all players and bullets, and orientations of players, to all clients on an `Unreliable` Renet channel, tagged with the number of the tick that was processed. More seriously consequential game events--in this case, just player death--are sent on a `ReliableOrdered` Renet channel. Everything else can go on the `Unreliable` channel. Even nonlethal hits can go on the `Unreliable` state channel; the health bar will just just to the correct value when the update comes. Note: send current health rather than "player took X amount of damge". In general, always sync the value not the delta on an `Unreliable` channel; the same goes for position, orientation, ammo, etc.

NOTE: The server should pre-allocate a fixed vector of `Option<PlayerInput>`. The size should be a power of 2 that is larger than the maximum expected latency window (e.g., 128 or 256). When a message arrives, calculate its index as `index = tick % buffer_size`.

## Client

Each iteration of its game loop, the client updates its estimate of the server clock. It then uses that estimate to calculate a tick number.

```rust
update_clock(session, network, dt); // Estimate server clock.

// 2. CALCULATE TARGET
// Where SHOULD we be?
let target_tick = (session.estimated_server_time / TICK_DT).floor() as u64 + LEAD_TICKS;

// 3. FEED THE ACCUMULATOR (Primary Fuel)
// `dt` is the actual time that's passed since the last tick.
// Use local system time for smoothness.
session.accumulator += dt;

// 4. APPLY NUDGE (Course Correction)
// Compare where we ARE vs where we SHOULD be.
if session.current_tick < target_tick {
    // We are falling behind! Add a tiny bit of "fake time" to catch up.
    // This will eventually trigger an extra physics step.
    session.accumulator += 0.0005; // Tunable value.
}
else if session.current_tick > target_tick {
    // We are running too fast! Remove a tiny bit of time.
    // This might delay the next physics step by one frame.
    session.accumulator -= 0.0005;
}

// 5. RUN PHYSICS
while session.accumulator >= TICK_DT { // Ideal tick duration.
    physics_step();
    session.accumulator -= TICK_DT;
    session.current_tick += 1;
}
```

It checks for inputs and pushes them as a `PlayerInput` (storing all current keypresses along with the tick number) to a `VecDeque` called `input_buffer`. It checks for messages from the server. The server should have sent an authoritative snapshot of the game state. This the client treats as its new baseline. First the client discards any inputs from ticks earlier than that of the baseline. Then sets its game state equal to the baseline ('reconciliation'), then--before rendering anything, purely in its physics simulation--replays its inputs for subsequent ticks from the baseline to the most recent input ('prediction'). Finally, it renders the result. (A refinement would be to smooth the transition from current position to the new estimate, but this is good enough for now.)

The client checks for new inputs and sends the most recent 4 inputs to the server on an `Unreliable` Renet channel. This redundancy increases the chance that the server will have inputs available for each tick it processes and not have to guess.

Note: To be consistent with the server, ensure the physics update uses a fixed timestep (e.g., 1.0/60.0) and not `get_frame_time()`.

REVISION: Here's what we can do, using the idea of a dynamic lead. Combining NTP (Network Time Protocol), defined by the clock estimate, with a clamped nudge, turns the game loop into what's known as a Control System (specifically a Proportional Controller with Saturation).

Here we combine NTP with with dynamic nudge and spike handling:

```rust
// --- CONSTANTS ---
const SERVER_TICK_RATE: f64 = 60.0;
const TICK_DT: f64 = 1.0 / SERVER_TICK_RATE;
// 3 Ticks (50ms) is a safe starting buffer.
// If your inputs arrive late on the server, increase this.
const JITTER_BUFFER_SECONDS: f64 = 0.050;

// --- THE LOOP ---

// 1. MEASURE REAL TIME
// We always measure raw time first.
let raw_delta_time = macroquad::time::get_frame_time() as f64;
let frame_duration = std::time::Duration::from_secs_f64(raw_delta_time);

// 2. UPDATE BASELINES
// A. Update the "radar" (Server Time Estimate).
// This keeps session.estimated_server_time aligned with the server's clock.
update_clock(&mut session, &mut network, frame_duration);

// B. Update the "road conditions" (RTT).
// We use asymmetric smoothing:
// - If RTT goes UP (lag spike), we adapt QUICKLY (0.1) to prevent input starvation.
// - If RTT goes DOWN (improvement), we adapt SLOWLY (0.01) to keep simulation stable.
let current_rtt = network.rtt();
let rtt_alpha = if current_rtt > session.smoothed_rtt { 0.1 } else { 0.01 };

// Simple linear interpolation.
// Encapsulate as `lerp(session.smoothed_rtt, renet.rtt(), alpha)`.
session.smoothed_rtt = session.smoothed_rtt * (1.0 - rtt_alpha) + current_rtt * rtt_alpha;

// 3. CALCULATE TARGET TIME
// This is the crucial formula.
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
while session.accumulator >= TICK_DT {
    // A. Process Inputs (Push to buffer, send to server).
    process_input(session.current_tick);

    // B. Run Physics Prediction.
    perform_tick(session.current_tick);

    // C. Advance State.
    session.accumulator -= TICK_DT;
    session.current_tick += 1;

    // Track our time using the fixed step to stay perfectly in sync with ticks.
    session.simulated_time += TICK_DT;
}

// 8. RENDER INTERPOLATION
let alpha = session.accumulator / TICK_DT;
render(alpha);

macroquad::window::next_frame().await;
```

The road metaphor:

- Road: The timeline.
- Radar: The logic finding the Server's position on that timeline.
- Road Conditions: The noise in the RTT that forces us to keep a larger following distance (Buffer).

So,

- The Server Truck is at Time 100.0s.
- The Client Scout Car looks at its radar (NTP/update_clock) to locate the Truck.
- The Client calculates the gap:
  - It takes 0.05s for fuel (inputs) to reach the Truck (smoothed_rtt / 2).
  - It adds 0.05s extra padding just in case (JITTER_BUFFER).
- The Target: The Client hits the gas until it is at Time 100.1s.
