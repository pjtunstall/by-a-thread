use std::time::{Duration, SystemTime, UNIX_EPOCH};

// If either of these functions panics because the system time is before the
// Unix epoch, try to enable automatic time synchronization (NTP) as follows:
//
// Linux: `timedatectl set-ntp true` (or `rc-service ntpd start` / `rc-service
// chronyd start` on non-systemd distros).
//
// Windows: Settings -> Time & language -> Date & time, enable 'Set time
// automatically'.
//
// macOS: System Settings -> General -> Date & Time, enable 'Set date and time
// automatically'.
pub fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before Unix epoch")
}

pub fn now_as_secs_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before Unix epoch")
        .as_secs_f64()
}
