use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH!") // If this problem occurs, open system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.
}
