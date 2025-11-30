# Plan for Local Player Netcode

We choose a tick frequency, 60Hz. We choose a lead, for now just a fixed number of ticks, erring on the side of slightly larger than we're likely to need: 4. The lead represents how far in the past the server will treat as the 'current' tick to be processed. It's necessary to account for the time it takes to receive inputs from the player. We want to make sure there's plenty of time for inputs to reach the server.

REVISION: 4 ticks at 60Hz is 66ms, which might be too small. Instead, use a dynamic lead: `rtt / 2 + jitter_buffer`. Every frame,

```rust
smoothed_rtt = lerp(smoothed_rtt, renet.rtt(), 0.01);
target = smoothed_rtt / tick_duration + 3;
```

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
const SERVER_TICK_RATE: f64 = 60.0;
const TICK_DT: f64 = 1.0 / SERVER_TICK_RATE;
const JITTER_BUFFER_SECONDS: f64 = 0.050; // ~3 ticks worth of safety

// ...

// 1. Get the target time from your NTP-style clock (Network Time Protocol).
let target_sim_time = session.estimated_server_time + JITTER_BUFFER_SECONDS;

// 2. Calculate where we currently are.
// Ideally, track this as a float. Alternatively: (tick as f64 * dt)
let current_sim_time = session.simulated_time;

// 3. Calculate Error.
let error = target_sim_time - current_sim_time;

// 4. THE HYBRID CHECK.
let adjustment = if error.abs() > 0.25 {
    // CASE A: The "Hard Snap".
    // We are more than 250ms off. The internet hiccuped.
    // Give up on smoothing. Teleport time immediately.
    // (You might want to fade the screen to black or show a connection icon here).
    println!("Resyncing clock...");
    error // Add the whole error instantly.
} else {
    // CASE B: The "Clamped Nudge" (Your logic).
    // 10% gain, clamped to 2ms limit.
    (error * 0.1).clamp(-0.002, 0.002)
};

// 5. Apply to Accumulator.
// We add the real frame time PLUS the adjustment.
session.accumulator += raw_delta_time + adjustment;

// 6. Run Physics.
while session.accumulator >= TICK_DT {
    perform_tick();
    session.accumulator -= TICK_DT;
    session.simulated_time += TICK_DT; // Or `session.current_tick += 1;`.
}

let alpha = accumulator / TICK_DT;
render(alpha);

macroquad::window::next_frame().await;
```
