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
    pub exit_coords: Option<(usize, usize)>,
    pub timer_duration: f32,
}

impl Default for InitialData {
    fn default() -> Self {
        Self {
            maze: Maze::new(Algorithm::Backtrack),
            players: Vec::new(),
            difficulty: 1,
            exit_coords: None,
            timer_duration: 360.0,
        }
    }
}

impl InitialData {
    pub fn new(usernames: &HashMap<u64, String>, colors: &HashMap<u64, Color>, level: u8) -> Self {
        let generator = match level {
            0 => Algorithm::BinaryTree,
            1 => Algorithm::RecursiveDivision,
            2 => Algorithm::VoronoiQueue,
            3 => Algorithm::Blobby,
            4 => Algorithm::VoronoiStack,
            5 => Algorithm::Prim,
            6 => Algorithm::Kruskal,
            7 => Algorithm::VoronoiRandom,
            8 => Algorithm::Backtrack,
            9 => Algorithm::Wilson,
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
            exit_coords = Some(maze.make_exit(solo_player_grid_coords));
            timer_duration = SOLO_TIMER_DURATION
        } else {
            exit_coords = None;
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
