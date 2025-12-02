use macroquad::prelude::Texture2D;

use super::world::maze::MazeExtension;
use shared::snapshot::Snapshot;

#[derive(Debug)]
pub struct Game {
    pub snapshot: Snapshot,
}

impl Game {
    pub fn new(snapshot: Snapshot) -> Self {
        Self { snapshot }
    }

    pub fn draw(&self, texture: &Texture2D) {
        let snapshot = &self.snapshot;
        snapshot.maze.draw(texture);
    }

    pub fn reconcile(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
    }
}
