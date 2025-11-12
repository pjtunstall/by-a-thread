pub mod auth;
pub mod chat;
pub mod net;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use renet::{ChannelConfig, ConnectionConfig, SendType};
use serde::{Deserialize, Serialize};

pub fn protocol_version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}

pub fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH!") // If this problem occurs, open system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.
}

pub fn connection_config() -> ConnectionConfig {
    let reliable_config = ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(100),
        },
    };

    let unreliable_config = ChannelConfig {
        channel_id: 1,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };

    let time_sync_config = ChannelConfig {
        channel_id: 2,
        max_memory_usage_bytes: 1 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };

    let client_channels_config = vec![reliable_config.clone(), unreliable_config.clone()];
    let server_channels_config = vec![reliable_config, unreliable_config, time_sync_config];

    ConnectionConfig {
        client_channels_config,
        server_channels_config,
        ..Default::default()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    /// Sent 10-20x/sec on an unreliable channel.
    /// Contains the server's time in seconds (f64).
    ServerTime(f64),

    /// Sent once on a reliable channel to start the countdown.
    /// Contains the exact server time (f64) when the countdown will end.
    CountdownStarted { end_time: f64 },
    // Add other messages here later, e.g.:
    // ChatMessage(String),
    // PlayerJoined(String),
}
