use std::collections::HashMap;

use rand::random_range;
use serde::{Deserialize, Serialize};

use crate::{
    maze::{self, Maze, maker::Algorithm},
    player::{self, Player},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snapshot {
    pub maze: Maze,
    pub players: Vec<Player>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            maze: Maze::new(Algorithm::Backtrack),
            players: Vec::new(),
        }
    }
}

// TODO: This was a placeholder for seeing an initial view of the maze, to be sent.
// over a reliable channel.
// In practice, the usernames won't need adding each time or the maze data sending.
// Distinguish between `InitialData` (maze and players), and `Snapshot`, which will
// just send the latest player data where it's changed (or for which player's it's)
// changed.
impl Snapshot {
    pub fn new(usernames: &HashMap<u64, String>, level: u8) -> Self {
        let generator = match level {
            1 => Algorithm::Backtrack,
            2 => Algorithm::Wilson,
            _ => Algorithm::Prim,
        };
        let maze = maze::Maze::new(generator);

        let mut spaces_remaining = maze.spaces.clone();
        let mut player_count: usize = 0;
        let players: Vec<Player> = usernames
            .clone()
            .into_iter()
            .map(|(client_id, username)| {
                let space_index = random_range(0..spaces_remaining.len());
                let (z, x) = spaces_remaining.remove(space_index);
                let start_position = maze
                    .position_from_grid_coordinates(player::HEIGHT, z, x)
                    .expect("failed to get start position from maze");
                let player = Player::new(
                    player_count,
                    client_id,
                    username.clone(),
                    start_position,
                    player::COLORS[player_count % player::COLORS.len()],
                );
                player_count += 1;
                player
            })
            .collect();

        Self { maze, players }
    }
}
