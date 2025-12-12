use macroquad::{color, prelude::*, window::clear_background};

use crate::{assets::Assets, game::world::maze::MazeExtension};
use common::{
    constants::INPUT_HISTORY_LENGTH,
    maze::Maze,
    player::{Player, PlayerInput},
    snapshot::InitialData,
};

#[derive(Debug)]
pub struct Game {
    pub local_player_index: usize,
    pub maze: Maze,
    pub players: Vec<Player>,
    // pub snapshot_buffer: [Snapshot; SNAPSHOT_BUFFER_LENGTH], // 16 broadcasts, 0.8s at 20Hz.
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

    pub fn update(&mut self) {
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
