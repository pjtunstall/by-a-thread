use common::{
    player::{Color, Player, PlayerInput, PlayerState},
    ring::NetworkBuffer,
};

pub const INPUT_BUFFER_LENGTH: usize = 128;

pub struct ServerPlayer {
    pub last_processed_tick: u64,
    pub last_input: PlayerInput,
    pub input_buffer: NetworkBuffer<PlayerInput, INPUT_BUFFER_LENGTH>,
    pub index: usize,
    pub client_id: u64,
    pub name: String,
    pub state: PlayerState,
    pub color: Color,
    pub status: Status,
    pub over_cap_strikes: u8,
    pub health: u8,
    pub last_fire_tick: Option<u64>,
    pub bullets_in_air: usize,
    pub exit_tick: Option<u64>,
}

impl ServerPlayer {
    pub fn new(player: Player, current_tick: u64) -> Self {
        let status = if player.disconnected {
            Status::Disconnected
        } else if player.health == 0 {
            Status::Dead
        } else {
            Status::Alive
        };

        Self {
            name: player.name,
            index: player.index,
            state: player.state,
            color: player.color,
            status,
            client_id: player.client_id,
            last_processed_tick: 0,
            last_input: PlayerInput::default(),
            input_buffer: NetworkBuffer::new(current_tick, current_tick),
            over_cap_strikes: 0,
            health: player.health,
            last_fire_tick: None,
            bullets_in_air: 0,
            exit_tick: None,
        }
    }
}

#[repr(u8)]
pub enum Status {
    Alive,
    Dead,
    Disconnected,
}
