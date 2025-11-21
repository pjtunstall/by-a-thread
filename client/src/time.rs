use crate::{
    net::{NetworkHandle, RenetNetworkHandle},
    session::ClientSession,
};
use shared::{self, net::AppChannel, protocol::ServerMessage};

pub fn update_estimated_server_time(session: &mut ClientSession, network: &mut RenetNetworkHandle) {
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
            Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
                let rtt = network.rtt();
                let one_way_latency = (rtt / 1000.0) / 2.0;
                let target_time = server_sent_time + one_way_latency;
                let delta = target_time - session.estimated_server_time;
                if delta.abs() > 1.0 {
                    session.estimated_server_time = target_time;
                } else {
                    let alpha = 0.1;
                    session.estimated_server_time += delta * alpha;
                }
            }
            _ => {}
        }
    }
}
