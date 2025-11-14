pub mod auth;
pub mod chat;
pub mod input;
pub mod math;
pub mod maze;
pub mod net;
pub mod player;
pub mod protocol;
pub mod time;

pub const MAZE_RADIUS: usize = 16; // Double and add one to get the width of the maze in grid cells, including edge walls. The reason for this calculation is to ensure an odd number of chars for the width. This lets us draw a nice map with equally thick edges, no matter the value of this parameter used to set its width.
pub const PLAYER_RADIUS: f32 = 8.0;
pub const PLAYER_HEIGHT: f32 = 24.0; // Height of the player's eye level from the ground.
