use bincode::{config::standard, serde::encode_to_vec};
use macroquad::prelude::*;

use common::player::PlayerInput;

pub const INPUT_HISTORY_LENGTH: usize = 256;

pub struct InputHistory {
    pub baseline_tick: u64, // Tick number of the last snapshot received from the server.
    pub history: [Option<PlayerInput>; INPUT_HISTORY_LENGTH],
}

impl InputHistory {
    pub fn new() -> Self {
        Self {
            baseline_tick: 0,
            history: [const { None }; INPUT_HISTORY_LENGTH],
        }
    }
}

pub fn player_input_as_bytes(input: &PlayerInput) -> Vec<u8> {
    encode_to_vec(input, standard()).expect("failed to encode player input")
}

pub fn player_input_from_keys() -> PlayerInput {
    PlayerInput {
        forward: is_key_down(KeyCode::W),
        backward: is_key_down(KeyCode::S),
        left: is_key_down(KeyCode::A),
        right: is_key_down(KeyCode::D),
        yaw_left: is_key_down(KeyCode::Left),
        yaw_right: is_key_down(KeyCode::Right),
        pitch_up: is_key_down(KeyCode::Up),
        pitch_down: is_key_down(KeyCode::Down),
    }
}
