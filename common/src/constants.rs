// Client:
pub const JITTER_SAFETY_MARGIN: f64 = 50.0; // Milliseconds.
pub const INPUT_HISTORY_LENGTH: usize = 256; // 256 ticks, ~4.3s at 60Hz.
pub const SNAPSHOT_BUFFER_LENGTH: usize = 16; // 16 broadcasts, 0.8s at 20Hz. Big safety margin in case we introduce a dynamic interpolation delay later.

// Server:
pub const INPUT_BUFFER_LENGTH: usize = 128;
pub const MAX_PLAYERS: usize = 10;
