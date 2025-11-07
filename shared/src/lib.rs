pub mod auth;
pub mod chat;
pub mod net;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        .expect("system clock set to a date before 1970") // If this problem occurs, open system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.
}
