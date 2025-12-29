use common::{
    player::{Color, Player, PlayerInput, PlayerState},
    ring::NetworkBuffer,
};

pub const INPUT_BUFFER_LENGTH: usize = 128;

pub struct ServerPlayer {
    pub last_processed_tick: u64,
    pub last_input: Option<PlayerInput>,
    pub input_buffer: NetworkBuffer<PlayerInput, INPUT_BUFFER_LENGTH>,
    pub index: usize,
    pub client_id: u64,
    pub name: String,
    pub state: PlayerState,
    pub color: Color,
    pub disconnected: bool,
    pub alive: bool,
}

impl ServerPlayer {
    pub fn new(player: Player) -> Self {
        Self {
            name: player.name,
            index: player.index,
            state: player.state,
            color: player.color,
            disconnected: player.disconnected,
            alive: player.alive,
            client_id: player.client_id,
            last_processed_tick: 0,
            last_input: None,
            input_buffer: NetworkBuffer::new(),
        }
    }
}
