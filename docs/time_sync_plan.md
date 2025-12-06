# Revised clock sync

Replace current EMA ...

```rust
use std::time::Duration;

use bincode::config::standard;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use common::{self, net::AppChannel, protocol::ServerMessage};

const MAX_REASONABLE_RTT: f64 = 1.0;

const HARD_SNAP_THRESHOLD: f64 = 1.0; // >1s: Just teleport.
const FAST_CATCHUP_THRESHOLD: f64 = 0.25; // >250ms: Speed up significantly.
const MODERATE_DRIFT_THRESHOLD: f64 = 0.05; // >50ms: Standard correction.

const ALPHA_FAST: f64 = 0.3; // Catch up quickly.
const ALPHA_NORMAL: f64 = 0.1; // Standard smoothing.
const ALPHA_JITTER: f64 = 0.03; // High damping for noise.

const MIN_CLOCK_CORRECTION_LIMIT: f64 = 0.01;
const MAX_CLOCK_CORRECTION_LIMIT: f64 = 0.05;
const CLOCK_CORRECTION_RATIO: f64 = 0.25; // How much of the the error is corrected per frame.

pub fn update_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    interval: Duration,
) {
    session.estimated_server_time += interval.as_secs_f64();

    let mut latest_message = None;
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        latest_message = Some(message); // ServerTime messages are sent at 20Hz (every 50ms).
    }
    let Some(message) = latest_message else {
        return;
    };

    match bincode::serde::decode_from_slice(&message, standard()) {
        Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
            let rtt = network.rtt();

            // Discard messages with massive RTT; their timing info is stale.
            if rtt > MAX_REASONABLE_RTT {
                return;
            }

            let target_time = server_sent_time + (rtt / 2.0);
            let delta = target_time - session.estimated_server_time;

            // Only snap if we are wildly off (> 1 second) or uninitialized, e.g. on startup.
            if session.estimated_server_time == 0.0 || delta.abs() > HARD_SNAP_THRESHOLD {
                session.estimated_server_time = target_time;
                println!("Hard sync: clock snapped to {}.", target_time);
                return;
            }

            session.estimated_server_time += correction(delta);
        }
        Err(e) => {
            eprintln!("Failed to deserialize ServerTime message: {}.", e);
        }
        _ => {}
    }
}

fn correction(delta: f64) -> f64 {
    // If we're off by a lot, go a bigger proportion of the way towards the target.
    let alpha = if delta.abs() > FAST_CATCHUP_THRESHOLD {
        ALPHA_FAST
    } else if delta.abs() > MODERATE_DRIFT_THRESHOLD {
        ALPHA_NORMAL
    } else {
        ALPHA_JITTER
    };

    let raw_correction = delta * alpha;

    // Limit the correction to a tighter range when there is less to correct.
    // Low error: prioritize smoothness. High error: prioritize speed.
    let dynamic_limit = (delta.abs() * CLOCK_CORRECTION_RATIO)
        .clamp(MIN_CLOCK_CORRECTION_LIMIT, MAX_CLOCK_CORRECTION_LIMIT);

    let correction = raw_correction.clamp(-dynamic_limit, dynamic_limit);

    correction
}

```

of `client/src/time.rs` with a system that aims towards values with the smallest RTT?

```rust
pub struct ClockSync {
    pub current_offset: f64,
    target_offset: f64,
    samples: VecDeque<Sample>,
}

struct Sample {
    rtt: f64,
    offset: f64,
}

impl ClockSync {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(64), // 3 seconds of history (at 20Hz).
            target_offset: 0.0,
            current_offset: 0.0,
        }
    }

    // 1. INPUT STEP: Finding the Target (The "Truth")
    pub fn process_packet(&mut self, server_time: f64, rtt: f64, local_time: f64) {
        // Calculate what the offset would be IF this packet were perfect.
        let latency = rtt / 2.0; // Rename message, since we're talking about Renet messages, but literal packets.
        let packet_offset = (server_time + latency) - local_time;

        // Add to window of samples.
        if self.samples.len() >= 64 { self.samples.pop_front(); }
        self.samples.push_back(Sample { rtt, offset: packet_offset });

        // THE GOLDEN RULE: The sample with the lowest RTT is the most accurate.
        // representation of the actual clock difference.
        if let Some(best) = self.samples.iter()
            .min_by(|a, b| a.rtt.partial_cmp(&b.rtt).unwrap())
        {
            self.target_offset = best.offset;
        }
    }

    // 2. UPDATE STEP: Moving the Clock. Supposedly this is the previous logic. Is it?
    pub fn update(&mut self, dt: f64) {
        // How far are we from the truth?
        let delta = self.target_offset - self.current_offset;

        // Gemini: --- HERE IS YOUR LOGIC ---
        // We apply your sophisticated damping/catchup logic here.
        // I adapted it slightly to apply per-frame (dt) rather than per-packet.

        // Me: It's clearly much streamlined. That might be a good thing, but should be acknowledged. Was my previous logic not also per frame?

        let correction_speed = self.calculate_correction_speed(delta);

        // Apply the correction
        self.current_offset += correction_speed * dt;
    }

    // Your logic, adapted for continuous time. Wasn't it already continuous time?
    fn calculate_correction_speed(&self, delta: f64) -> f64 {
        const HARD_SNAP: f64 = 1.0;
        const FAST_CATCHUP: f64 = 0.25;

        // If we are wildly off, snap immediately (teleport).
        if delta.abs() > HARD_SNAP {
            return delta / 0.016; // Move instantly in one frame
        }

        // Your "Alpha" logic becomes "Speed" logic in a continuous loop.
        // How many units of offset do we correct per second?
        let drift_rate = if delta.abs() > FAST_CATCHUP {
            5.0 // Fix big errors in ~0.2 seconds
        } else if delta.abs() > 0.05 {
            1.0 // Fix moderate drift in ~1 second
        } else {
            0.1 // Fix tiny jitter very slowly (10 seconds) to remain smooth
        };

        delta * drift_rate
    }
}
```
