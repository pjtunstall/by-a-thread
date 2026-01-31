use std::collections::HashMap;

use rand;
use serde::{Deserialize, Serialize};

use crate::{
    constants::{BATTLE_TIMER_DURATION, SOLO_TIMER_DURATION},
    maze::{self, Maze, maker::Algorithm},
    player::{self, Color, Player, WirePlayerLocal, WirePlayerRemote},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub remote: Vec<WirePlayerRemote>,
    pub local: WirePlayerLocal,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InitialData {
    pub maze: Maze,
    pub players: Vec<Player>,
    pub difficulty: u8,
    pub exit_coords: (usize, usize),
    pub timer_duration: f32,
}

impl Default for InitialData {
    fn default() -> Self {
        Self {
            maze: Maze::new(Algorithm::Backtrack),
            players: Vec::new(),
            difficulty: 1,
            exit_coords: (0, 0),
            timer_duration: 360.0,
        }
    }
}

impl InitialData {
    pub fn new(usernames: &HashMap<u64, String>, colors: &HashMap<u64, Color>, level: u8) -> Self {
        let generator = match level {
            1 => Algorithm::Backtrack,
            2 => Algorithm::Wilson,
            3 => Algorithm::Prim,
            _ => Algorithm::Backtrack,
        };
        let mut maze = maze::Maze::new(generator);

        let mut spaces_remaining = maze.spaces.clone();
        let mut player_count: usize = 0;
        let players: Vec<Player> = usernames
            .clone()
            .into_iter()
            .map(|(client_id, username)| {
                let space_index = rand::random_range(0..spaces_remaining.len());
                let (z, x) = spaces_remaining.remove(space_index);
                let start_position = maze
                    .position_from_grid_coordinates(player::HEIGHT, z, x)
                    .expect("failed to get start position from maze");
                let color = colors
                    .get(&client_id)
                    .copied()
                    .unwrap_or(player::COLORS[player_count % player::COLORS.len()]);
                let player = Player::new(
                    player_count,
                    client_id,
                    username.clone(),
                    start_position,
                    color,
                );
                player_count += 1;
                player
            })
            .collect();

        let exit_coords = maze.pick_exit_coords();
        let is_solo = player_count == 1;
        let timer_duration = if is_solo {
            let (exit_z, exit_x) = exit_coords;
            maze.grid[exit_z][exit_x] = 0;
            maze.spaces.push(exit_coords);
            SOLO_TIMER_DURATION
        } else {
            BATTLE_TIMER_DURATION
        };

        Self {
            maze,
            players,
            difficulty: level,
            exit_coords,
            timer_duration,
        }
    }
}
