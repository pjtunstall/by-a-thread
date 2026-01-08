use std::fmt;

use macroquad::{color, prelude::*, window::clear_background};

use crate::game::world::maze::{MazeExtension, MazeMeshes};
use common::{
    constants::INPUT_HISTORY_LENGTH,
    maze::Maze,
    player::{Player, PlayerInput},
    ring::Ring,
    snapshot::InitialData,
};

pub struct Game {
    pub local_player_index: usize,
    pub maze: Maze,
    pub maze_meshes: MazeMeshes,
    pub players: Vec<Player>,
    // pub snapshot_buffer: [Snapshot; SNAPSHOT_BUFFER_LENGTH], // 16 broadcasts, 0.8s at 20Hz.
    pub input_history: Ring<PlayerInput, INPUT_HISTORY_LENGTH>, // 256: ~4.3s at 60Hz.
}

impl Game {
    pub fn new(
        local_player_index: usize,
        initial_data: InitialData,
        maze_meshes: MazeMeshes,
    ) -> Self {
        Self {
            local_player_index,
            maze: initial_data.maze,
            maze_meshes,
            players: initial_data.players,
            input_history: Ring::new(),
        }
    }

    pub fn update(&mut self) {
        // TODO: Reconciliation and prediction.
    }

    pub fn draw(&self, _alpha: f64) {
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

        self.maze.draw(&self.maze_meshes);
    }
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
            .field("local_player_index", &self.local_player_index)
            .field("maze", &self.maze)
            .field("maze_meshes", &self.maze_meshes)
            .field("players", &self.players)
            .field("input_history", &self.input_history)
            .finish()
    }
}
