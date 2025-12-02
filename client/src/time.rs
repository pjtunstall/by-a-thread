use std::time::Duration;

use bincode::config::standard;

use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use common::{self, net::AppChannel, protocol::ServerMessage};

// TODO: Document, mock, and test.
// ServerTime messages are sent at 20Hz, every 50ms.

// Thresholds
const HARD_SNAP_THRESHOLD: f64 = 1.0; // >1s: Just teleport.
const FAST_CATCHUP_THRESHOLD: f64 = 0.25; // >250ms: Speed up significantly.
const MODERATE_DRIFT_THRESHOLD: f64 = 0.05; // >50ms: Standard correction.

// Alphas
const ALPHA_FAST: f64 = 0.3; // Catch up quickly.
const ALPHA_NORMAL: f64 = 0.1; // Standard smoothing.
const ALPHA_JITTER: f64 = 0.03; // High damping for noise.

// Speed Limits
const BASE_CLOCK_CORRECTION_LIMIT: f64 = 0.01; // Minimum nudge.
const MAX_CLOCK_CORRECTION_LIMIT: f64 = 0.05; // Max nudge - increased slightly for fast catchup.
const CLOCK_CORRECTION_RATIO: f64 = 0.25; // How much of the the error is corrected per frame.

const MAX_REASONABLE_RTT: f64 = 1.0;

pub fn update_clock(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
    interval: Duration,
) {
    session.estimated_server_time += interval.as_secs_f64();

    let mut latest_message = None;
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        latest_message = Some(message);
    }
    let Some(message) = latest_message else {
        return;
    };

    match bincode::serde::decode_from_slice(&message, standard()) {
        Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
            let rtt = network.rtt();

            // Discard messages with massive RTT; their timing info is stale.
            if rtt > MAX_REASONABLE_RTT {
                // Optional: Log once in a while, but don't spam.
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

    // Limit the correction to a tighter range when there is less to correct?!
    // Low error: prioritize smoothness. High error: prioritize speed.
    let dynamic_limit = (delta.abs() * CLOCK_CORRECTION_RATIO)
        .clamp(BASE_CLOCK_CORRECTION_LIMIT, MAX_CLOCK_CORRECTION_LIMIT);

    raw_correction.clamp(-dynamic_limit, dynamic_limit)
}
