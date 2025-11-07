use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use renet_netcode::ConnectToken;

pub fn default_server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

pub fn create_connect_token(
    current_time: Duration,
    protocol_id: u64,
    client_id: u64,
    server_addr: SocketAddr,
    private_key: &[u8; 32],
) -> ConnectToken {
    ConnectToken::generate(
        current_time,
        protocol_id,
        3600,
        client_id,
        15,
        vec![server_addr],
        None,
        private_key,
    )
    .expect("failed to generate token")
}
