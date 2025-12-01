use macroquad::prelude::Texture2D;

use super::world::maze::MazeExtension;
use shared::{maze::Maze, player::Player};

#[derive(Debug)]
pub struct Game {
    pub maze: Maze,
    pub players: Vec<Player>,
}

impl Game {
    pub fn new(maze: Maze, players: Vec<Player>) -> Self {
        Self { maze, players }
    }

    pub fn draw(&self, texture: &Texture2D) {
        self.maze.draw(texture);
    }
}
