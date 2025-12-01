use shared::player::{Player, PlayerInput};

pub const INPUT_BUFFER_LENGTH: usize = 128;

pub struct ServerPlayer {
    pub shared: Player,
    pub last_processed_tick: u64,
    pub last_input: Option<PlayerInput>,
    pub input_buffer: [PlayerInput; INPUT_BUFFER_LENGTH],
}
