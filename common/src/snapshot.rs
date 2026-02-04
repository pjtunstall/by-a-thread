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
            0 => Algorithm::RecursiveDivision,
            1 => Algorithm::Backtrack,
            2 => Algorithm::VoronoiStack,
            3 => Algorithm::BinaryTree,
            4 => Algorithm::Wilson,
            5 => Algorithm::Kruskal,
            6 => Algorithm::Blobby,
            7 => Algorithm::VoronoiRandom,
            8 => Algorithm::Prim,
            9 => Algorithm::VoronoiQueue,
            _ => Algorithm::Backtrack,
        };
        let mut maze = maze::Maze::new(generator);

        let mut solo_player_grid_coords = (0, 0);

        let mut spaces_remaining = maze.spaces.clone();
        let mut player_count: usize = 0;
        let players: Vec<Player> = usernames
            .clone()
            .into_iter()
            .map(|(client_id, username)| {
                let space_index = rand::random_range(0..spaces_remaining.len());
                let (z, x) = spaces_remaining.remove(space_index);
                solo_player_grid_coords = (z, x);
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

        let exit_coords;
        let timer_duration;

        let is_solo = player_count == 1;
        if is_solo {
            exit_coords = maze.make_exit(solo_player_grid_coords);
            timer_duration = SOLO_TIMER_DURATION
        } else {
            exit_coords = (999, 999);
            timer_duration = BATTLE_TIMER_DURATION;
        }

        Self {
            maze,
            players,
            difficulty: level,
            exit_coords,
            timer_duration,
        }
    }
}
