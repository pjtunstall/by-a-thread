use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use bincode::config::standard;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::{ClientSession, ClockSample},
};
use common::{net::AppChannel, protocol::ServerMessage};

const SAMPLE_WINDOW_SIZE: usize = 30;
const HARD_SNAP_THRESHOLD: f64 = 1.0;
const ALPHA_SPEED_UP: f64 = 0.15;
const ALPHA_SLOW_DOWN: f64 = 0.02;
const DEADZONE_THRESHOLD: f64 = 0.002;

pub fn update_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    interval: Duration,
) {
    session.estimated_server_time += interval.as_secs_f64();

    let now_seconds = get_monotonic_seconds();

    // Drain pending messages, append samples, then trim the window.
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        if let Ok((ServerMessage::ServerTime(server_sent_time), _)) =
            bincode::serde::decode_from_slice(&message, standard())
        {
            let rtt = network.rtt();

            // Only add valid samples.
            if rtt > 0.0 {
                session.clock_samples.push_back(ClockSample {
                    server_time: server_sent_time,
                    client_receive_time: now_seconds,
                    rtt,
                });

                // Maintain window size.
                if session.clock_samples.len() > SAMPLE_WINDOW_SIZE {
                    session.clock_samples.pop_front();
                }
            }
        }
    }

    if session.clock_samples.is_empty() {
        return;
    }

    // We assume the sample with the lowest RTT is the one least affected by network jitter.
    let best_sample = match session
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
    let error = target_server_time - session.estimated_server_time;

    // Hard snap (teleport if wildly off).
    if session.estimated_server_time == 0.0 || error.abs() > HARD_SNAP_THRESHOLD {
        session.estimated_server_time = target_server_time;
        println!("Hard sync: clock snapped to {:.4}.", target_server_time);
        return;
    }

    // Deadzone (prevent micro-stutter).
    if error.abs() < DEADZONE_THRESHOLD {
        return;
    }

    // Asymmetric smoothing: speed up fast, slow down slowly.
    let alpha = if error > 0.0 {
        ALPHA_SPEED_UP
    } else {
        ALPHA_SLOW_DOWN
    };

    session.estimated_server_time += error * alpha;
}

// Returns a monotonic f64 time source relative to app start.
fn get_monotonic_seconds() -> f64 {
    static START_TIME: OnceLock<Instant> = OnceLock::new();
    let start = START_TIME.get_or_init(Instant::now);
    start.elapsed().as_secs_f64()
}
