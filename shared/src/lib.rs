pub mod auth;
pub mod chat;
pub mod net;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
