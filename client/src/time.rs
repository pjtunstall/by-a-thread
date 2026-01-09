use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use bincode::config::standard;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::{ClientSession, ClockSample},
};
use common::{constants::TICK_SECS, net::AppChannel, protocol::ServerMessage};

const SAMPLE_WINDOW_SIZE: usize = 30;
const HARD_SNAP_THRESHOLD: f64 = 1.0;
const ALPHA_SPEED_UP: f64 = 0.15;
const ALPHA_SLOW_DOWN: f64 = 0.02;
const DEADZONE_THRESHOLD: f64 = 0.002;
const RTT_ALPHA_SPIKE: f64 = 0.1;
const RTT_ALPHA_IMPROVEMENT: f64 = 0.01;

// Three ticks (50ms) is probably a safe starting buffer.
// If inputs arrive late on the server, increase this.
const JITTER_SAFETY_MARGIN: f64 = 0.05; // Consider raising to 4 ticks?

pub fn estimate_server_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    dt: Duration,
) {
    // Always advance time by the frame delta.
    // This keeps the game smooth between network packets.
    session.clock.estimated_server_time += dt.as_secs_f64();

    let now_seconds = get_monotonic_seconds();

    let mut latest_rtt = None;

    // Drain pending messages, append samples, then trim the window.
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        if let Ok((ServerMessage::ServerTime(server_sent_time), _)) =
            bincode::serde::decode_from_slice(&message, standard())
        {
            let rtt = network.rtt();

            // Only add valid samples.
            if !rtt.is_nan() && rtt > 0.0 {
                session.clock.clock_samples.push_back(ClockSample {
                    server_time: server_sent_time,
                    client_receive_time: now_seconds,
                    rtt,
                });
                latest_rtt = Some(rtt);

                // Maintain window size.
                if session.clock.clock_samples.len() > SAMPLE_WINDOW_SIZE {
                    session.clock.clock_samples.pop_front();
                }
            }
        }
    }

    if session.clock.clock_samples.is_empty() {
        return;
    }

    // We assume the sample with the lowest RTT is the one least affected by network jitter.
    let best_sample = match session
        .clock
        .clock_samples
        .iter()
        .filter(|s| s.rtt.is_finite())
        .min_by(|a, b| a.rtt.partial_cmp(&b.rtt).unwrap())
    {
        Some(sample) => sample,
        None => {
            println!("No usable samples, e.g. all samples are NaN?");
            return;
        }
    };

    let age_of_sample = now_seconds - best_sample.client_receive_time;
    let latency_estimate = best_sample.rtt / 2.0;
    let target_server_time = best_sample.server_time + latency_estimate + age_of_sample;
    let error = target_server_time - session.clock.estimated_server_time;

    // Hard snap (teleport if wildly off).
    if session.clock.estimated_server_time == 0.0 || error.abs() > HARD_SNAP_THRESHOLD {
        session.clock.estimated_server_time = target_server_time;
        println!("Hard sync: clock snapped to {:.4}.", target_server_time);
        return;
    }

    // Deadzone (prevent micro-stutter).
    if error.abs() < DEADZONE_THRESHOLD {
        return;
    }

    // Asymmetric smoothing: speed up fast, slow down slowly.
    let clock_alpha = if error > 0.0 {
        ALPHA_SPEED_UP
    } else {
        ALPHA_SLOW_DOWN
    };
    session.clock.estimated_server_time += error * clock_alpha;

    if let Some(rtt) = latest_rtt.filter(|rtt| rtt.is_finite() && *rtt > 0.0) {
        if session.clock.smoothed_rtt == 0.0 {
            session.clock.smoothed_rtt = rtt;
        } else {
            let rtt_alpha = if rtt > session.clock.smoothed_rtt {
                RTT_ALPHA_SPIKE
            } else {
                RTT_ALPHA_IMPROVEMENT
            };
            session.clock.smoothed_rtt =
                session.clock.smoothed_rtt * (1.0 - rtt_alpha) + rtt * rtt_alpha;
        }
    }
}

// Target = "what time is it now" + "travel time" + "safety margin".
pub fn calculate_target_time(smoothed_rtt: f64, estimated_server_time: f64) -> f64 {
    let travel_time = smoothed_rtt / 2.0;
    estimated_server_time + travel_time + JITTER_SAFETY_MARGIN
}

pub fn calculate_target_tick(target_time: f64) -> u64 {
    (target_time / TICK_SECS).floor() as u64
}

// TODO: Decide: is this necessary? Of so, is it correct?
pub fn calculate_initial_tick(estimated_server_time: f64) -> u64 {
    (estimated_server_time / TICK_SECS).floor() as u64
}

// Returns (accumulated_time, simulated_time).
pub fn smooth_dt(continuous_sim_time: f64, target_time: f64, frame_dt: f64) -> f64 {
    const HARD_SNAP_THRESHOLD: f64 = 0.25;
    const NUDGE_CLAMP: f64 = 0.002;

    let error = target_time - continuous_sim_time;

    // Large desync: snap the clock by the full error.
    if error.abs() > HARD_SNAP_THRESHOLD {
        return error;
    }

    // Small desync: nudge a fraction, clamped to avoid visible stutter.
    let adjustment = (error * 0.1).clamp(-NUDGE_CLAMP, NUDGE_CLAMP);
    frame_dt + adjustment
}

// Returns a monotonic f64 time source relative to app start.
fn get_monotonic_seconds() -> f64 {
    static START_TIME: OnceLock<Instant> = OnceLock::new();
    let start = START_TIME.get_or_init(Instant::now);
    start.elapsed().as_secs_f64()
}
