use macroquad::prelude::Texture2D;

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

    pub fn draw(&self, texture: &Texture2D) {
        let snapshot = &self.snapshot;
        snapshot.maze.draw(texture);
    }

    pub fn reconcile(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
    }
}
