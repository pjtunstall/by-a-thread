use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use shared::{self, net::AppChannel, protocol::ServerMessage};

pub fn update_estimated_server_time(session: &mut ClientSession, network: &mut RenetNetworkHandle) {
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
