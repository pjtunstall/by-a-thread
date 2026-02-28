use std::{net::SocketAddr, time::SystemTime, time::UNIX_EPOCH};

use common::constants::MAX_SESSION_DURATION;
use rand::TryRngCore;
use renet_netcode::ConnectToken;

pub fn private_key() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("`getrandom` failed; failed to generate private key");
    bytes
}

pub fn create_connect_token(
    server_host: std::net::IpAddr,
    port: u16,
    client_id: u64,
    private_key: &[u8; 32],
) -> ConnectToken {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch");
    let protocol_id = common::protocol::protocol_id();
    let server_address = SocketAddr::new(server_host, port);

    ConnectToken::generate(
        current_time,
        protocol_id,
        MAX_SESSION_DURATION,
        client_id,
        15, // Timeout after 15 seconds.
        vec![server_address],
        None,
        private_key,
    )
    .expect("failed to generate token")
}
