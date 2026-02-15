use std::time::Instant;

use base64::Engine;
use uuid::Uuid;

use crate::auth;

pub struct Game {
    pub id: Uuid,
    pub port: u16,
    pub player_count: u8,
    pub connect_tokens: Vec<String>,
    pub start_time: Instant,
}

impl Game {
    pub fn new(
        server_host: std::net::IpAddr,
        port: u16,
        player_count: u8,
        private_key: [u8; 32],
    ) -> Self {
        let mut connect_tokens = Vec::new();

        for _ in 0..player_count {
            let connect_token = auth::create_connect_token(server_host, port, &private_key);
            let mut bytes = Vec::new();
            connect_token
                .write(&mut bytes)
                .expect("failed to write token");
            let connect_token_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
            connect_tokens.push(connect_token_str);
        }

        Self {
            id: Uuid::new_v4(),
            port,
            player_count,
            connect_tokens,
            start_time: Instant::now(),
        }
    }

    pub fn get_token(&mut self) -> Option<String> {
        self.connect_tokens.pop()
    }
}
