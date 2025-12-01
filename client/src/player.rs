use macroquad::prelude::*;

use shared::player::{Player, PlayerInput};

pub const INPUT_HISTORY_LENGTH: usize = 256;

pub struct ClientPlayer {
    pub shared: Player,
    pub interpolation_start: Vec3,
    pub prediction_age: u32,
    pub input_history: [PlayerInput; INPUT_HISTORY_LENGTH],
}
