use macroquad::prelude::*;

use common::player::PlayerInput;

// TODO: Check if I need to attend to this: "Macroquad usually handles loss of
// focus, but you can explicitly check `miniquad::window::order_quit_event()` or
// similar focus events to force-clear your local input state."
pub fn player_input_from_keys(sim_tick: u64) -> PlayerInput {
    PlayerInput {
        sim_tick,
        forward: is_key_down(KeyCode::W),
        backward: is_key_down(KeyCode::S),
        left: is_key_down(KeyCode::A),
        right: is_key_down(KeyCode::D),
        yaw_left: is_key_down(KeyCode::Left),
        yaw_right: is_key_down(KeyCode::Right),
        pitch_up: is_key_down(KeyCode::Up),
        pitch_down: is_key_down(KeyCode::Down),
        fire: is_key_down(KeyCode::Space),
    }
}
