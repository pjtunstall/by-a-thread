use macroquad::prelude::*;

use shared::player::{Player, PlayerInput};

pub struct ClientPlayer {
    pub shared: Player,
    pub interpolation_start: Vec3,
    pub prediction_age: u32,
    pub input_history: [PlayerInput; 256],
}
