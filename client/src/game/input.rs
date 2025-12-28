use macroquad::prelude::*;

use common::player::PlayerInput;

pub fn player_input_from_keys(target_tick: u64) -> PlayerInput {
    PlayerInput {
        target_tick,
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
