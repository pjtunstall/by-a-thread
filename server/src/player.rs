use common::player::{Player, PlayerInput};

pub const INPUT_BUFFER_LENGTH: usize = 128;

pub struct ServerPlayer {
    pub common: Player,
    pub last_processed_tick: u64, // Assert that this is the same as the tick id of last_input.
    pub last_input: Option<PlayerInput>,
    pub input_buffer: [PlayerInput; INPUT_BUFFER_LENGTH],
}
