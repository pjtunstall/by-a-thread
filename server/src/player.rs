use shared::player::{Player, PlayerInput};

pub struct ServerPlayer {
    pub shared: Player,
    pub last_processed_tick: u64,
    pub last_input: Option<PlayerInput>,
    pub input_buffer: [PlayerInput; 128],
}
