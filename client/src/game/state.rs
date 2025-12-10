use macroquad::{color, prelude::*, window::clear_background};

use crate::game::input::InputHistory;

use super::world::maze::MazeExtension;
use common::snapshot::Snapshot;

#[derive(Debug)]
pub struct Game {
    pub snapshot: Snapshot,
    pub input_history: InputHistory,
}

impl Game {
    pub fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            input_history: InputHistory::new(),
        }
    }

    pub fn draw(&self, texture: &Texture2D, position: Vec3) {
        clear_background(color::BEIGE);

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
        snapshot.maze.draw(texture);
    }

    pub fn reconcile(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
    }
}
