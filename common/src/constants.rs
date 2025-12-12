// Client:
pub const JITTER_SAFETY_MARGIN: f64 = 50.0; // Milliseconds.
pub const INPUT_HISTORY_LENGTH: usize = 256; // 256 ticks, ~4.3s at 60Hz.
pub const SNAPSHOT_BUFFER_LENGTH: usize = 16; // 16 broadcasts, 0.8s at 20Hz. Big safety margin in case we introduce a dynamic interpolation delay later.

// Common:
// We use this approximation to be consistent with TICK_MICROS.
pub const TICK_SECS: f32 = 1_000_000.0 * 16667.0; // ~ 1 / 60.0, used in `common::player` for `update`.

// Server:
pub const INPUT_BUFFER_LENGTH: usize = 128;
pub const MAX_PLAYERS: usize = 10;
pub const TICK_MICROS: u64 = 16667; // Used in `server::run` to manage loop. 
pub const BROADCAST_PER_MILLIS: u64 = 50; // server::run
