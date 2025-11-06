use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use renet_netcode::ConnectToken;

pub fn client_private_key() -> [u8; 32] {
    [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ]
}

pub fn default_server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

pub fn protocol_version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .expect("Failed to parse major version")
}

pub fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock appears to be incorrect--it's set to a date before 1970!")
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
    .expect("Failed to generate token")
}
