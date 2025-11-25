use std::collections::HashMap;

use macroquad::prelude::Texture2D;

use super::world::maze::MazeExtension;
use shared::{maze::Maze, player::Player};

#[derive(Debug)]
pub struct Game {
    pub maze: Maze,
    pub players: HashMap<u64, Player>,
}

impl Game {
    pub fn draw(&self, texture: &Texture2D) {
        self.maze.draw(texture);
    }
}
