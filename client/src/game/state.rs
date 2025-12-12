use macroquad::{color, prelude::*, window::clear_background};

use crate::{
    assets::Assets,
    game::{
        input::{player_input_as_bytes, player_input_from_keys},
        world::maze::MazeExtension,
    },
    net::NetworkHandle,
};
use common::{
    constants::INPUT_HISTORY_LENGTH,
    maze::Maze,
    net::AppChannel,
    player::{Player, PlayerInput},
    snapshot::InitialData,
};

#[derive(Debug)]
pub struct Game {
    pub local_player_index: usize,
    pub maze: Maze,
    pub players: Vec<Player>,
    // pub snapshot_buffer: [Snapshot; SNAPSHOT_BUFFER_LENGTH], // 16:
    pub input_history: [PlayerInput; INPUT_HISTORY_LENGTH], // 256: ~4.3s at 60Hz.
}

impl Game {
    pub fn new(local_player_index: usize, initial_data: InitialData) -> Self {
        Self {
            local_player_index,
            maze: initial_data.maze,
            players: initial_data.players,
            input_history: [PlayerInput::default(); 256],
        }
    }

    pub fn update(&mut self, network: &mut dyn NetworkHandle) {
        // TODO: Replace this placeholder with actual `current_tick`.
        let current_tick: u16 = 0;
        let tick_index_u16 = current_tick % (INPUT_HISTORY_LENGTH as u16 - 1);
        let tick_index = tick_index_u16 as usize;

        let player_input = player_input_from_keys();
        let message = player_input_as_bytes(&player_input);
        network.send_message(AppChannel::Unreliable, message);
        self.input_history[tick_index] = player_input;

        // TODO: Replace the following placeholder positioning with full reconciliation and prediction logic.
    }

    pub fn draw(&self, assets: &Assets) {
        clear_background(color::BEIGE);

        let position = self.players[self.local_player_index].state.position;

        let yaw: f32 = 0.0;
        let pitch: f32 = 0.1;

        set_camera(&Camera3D {
            position,
            target: position
                + vec3(
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    yaw.cos() * pitch.cos(),
                ),
            up: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        });

        self.maze.draw(&assets.wall_texture);
    }
}
