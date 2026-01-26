use std::time::Duration;

// How long the last player remaining has to escape.
pub const ESCAPE_DURATION: f32 = 90.0;

// Client:
pub const JITTER_SAFETY_MARGIN: f64 = 50.0; // Milliseconds.
pub const INPUT_HISTORY_LENGTH: usize = 256; // 256 ticks, ~4.3s at 60Hz.
pub const SNAPSHOT_BUFFER_LENGTH: usize = 16; // 16 broadcasts, 0.8s at 20Hz. Big safety margin in case we introduce a dynamic interpolation delay later.

// Tick-related:
pub const TICK_RATE: f64 = 60.0;
pub const TICK_SECS: f64 = 1.0 / TICK_RATE;
pub const TICK_SECS_F32: f32 = TICK_SECS as f32; // Used in `common::player` for `update`.
// The following `Duration`s are used by server to manage its loop.
pub const TICK_MICROS: u64 = (TICK_SECS * 1_000_000.0 + 0.5) as u64;
pub const IDEAL_TICK_DURATION: Duration = Duration::from_micros(TICK_MICROS);
pub const TICKS_PER_BROADCAST: u64 = 3;
pub const BROADCAST_MICROS: u64 = TICKS_PER_BROADCAST * TICK_MICROS;
pub const BROADCAST_INTERVAL: Duration = Duration::from_micros(BROADCAST_MICROS); // 50ms.

// Server:
pub const INPUT_BUFFER_LENGTH: usize = 128; // 128 ticks, ~2.1s at 60Hz.
pub const MAX_PLAYERS: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_array_lengths_are_powers_of_two() {
        // INPUT_BUFFER (server).
        assert!(
            INPUT_BUFFER_LENGTH != 0,
            "INPUT_BUFFER_LENGTH should not be 0"
        );
        assert!(
            INPUT_BUFFER_LENGTH & (INPUT_BUFFER_LENGTH - 1) == 0,
            "INPUT_BUFFER_LENGTH should be a power of 2"
        );

        // INPUT_HISTORY (client).
        assert!(
            INPUT_HISTORY_LENGTH != 0,
            "INPUT_HISTORY_LENGTH should not be 0"
        );
        assert!(
            INPUT_HISTORY_LENGTH & (INPUT_HISTORY_LENGTH - 1) == 0,
            "INPUT_HISTORY_LENGTH should be a power of 2"
        );

        // SNAPSHOT_BUFFER (client).
        assert!(
            SNAPSHOT_BUFFER_LENGTH != 0,
            "SNAPSHOT_BUFFER_LENGTH should not be 0"
        );
        assert!(
            SNAPSHOT_BUFFER_LENGTH & (SNAPSHOT_BUFFER_LENGTH - 1) == 0,
            "SNAPSHOT_BUFFER_LENGTH should be a power of 2"
        );
    }
}
