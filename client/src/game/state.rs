use macroquad::{color, prelude::*, window::clear_background};

use crate::game::input::InputHistory;

use crate::{
    assets::Assets,
    game::{
        input::{INPUT_HISTORY_LENGTH, player_input_as_bytes, player_input_from_keys},
        world::maze::MazeExtension,
    },
    net::NetworkHandle,
};
use common::net::AppChannel;
use common::snapshot::Snapshot;

#[derive(Debug)]
pub struct Game {
    pub local_player_index: usize,
    pub snapshot: Snapshot,
    pub input_history: InputHistory,
}

impl Game {
    pub fn new(local_player_index: usize, snapshot: Snapshot) -> Self {
        Self {
            local_player_index,
            snapshot,
            input_history: InputHistory::new(),
        }
    }

    pub fn update(&mut self, network: &mut dyn NetworkHandle) {
        // TODO: Replace this placeholder with actual `current_tick`.
        // We'll need to coerce the tick to usize, unless we can make
        // that it's original type.
        let current_tick = 0;

        let player_input = player_input_from_keys();
        let message = player_input_as_bytes(&player_input);
        network.send_message(AppChannel::Unreliable, message);
        self.input_history.history[current_tick % (INPUT_HISTORY_LENGTH - 1)] = Some(player_input);

        // TODO: Replace the following placeholder positioning with full reconciliation and prediction logic.
    }

    pub fn draw(&self, assets: &Assets) {
        clear_background(color::BEIGE);

        let position = self.snapshot.players[self.local_player_index]
            .state
            .position;

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

        let snapshot = &self.snapshot;
        snapshot.maze.draw(&assets.wall_texture);
    }

    pub fn reconcile(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
    }
}
