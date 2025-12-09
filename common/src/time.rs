use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TICK_RATE: f32 = 60.0;
pub const TICK_MICROS: u64 = 16667;
pub const TICK_SECS: f32 = 1.0 / 60.0;

pub fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch") // If this problem occurs, open system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.
}
