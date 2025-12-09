use macroquad::prelude::*;

use common::{
    self,
    input::{
        A_KEY_HELD, A_KEY_PRESSED, D_KEY_HELD, D_KEY_PRESSED, DOWN_KEY_HELD, DOWN_KEY_PRESSED,
        LEFT_KEY_HELD, LEFT_KEY_PRESSED, RIGHT_KEY_HELD, RIGHT_KEY_PRESSED, S_KEY_HELD,
        S_KEY_PRESSED, SPACE_KEY_HELD, SPACE_KEY_PRESSED, UP_KEY_HELD, UP_KEY_PRESSED, W_KEY_HELD,
        W_KEY_PRESSED,
    },
};

pub fn input_to_bytes() -> Vec<u8> {
    let bitfield = bitfield_from_input();
    common::input::bitfield_to_bytes(bitfield)
}

// By 'pressed', Macroquad means started being pressed this frame.
// By 'down', it means being held down this frame. I've called this
// 'held' to avoid clashing with the names of the 'up' and 'down'
// arrow keys.
fn bitfield_from_input() -> u32 {
    let mut result = 0u32;

    if is_key_down(KeyCode::W) {
        result |= W_KEY_HELD;
    }
    if is_key_pressed(KeyCode::W) {
        result |= W_KEY_PRESSED;
    }
    if is_key_down(KeyCode::A) {
        result |= A_KEY_HELD;
    }
    if is_key_pressed(KeyCode::A) {
        result |= A_KEY_PRESSED;
    }
    if is_key_down(KeyCode::S) {
        result |= S_KEY_HELD;
    }
    if is_key_pressed(KeyCode::S) {
        result |= S_KEY_PRESSED;
    }
    if is_key_down(KeyCode::D) {
        result |= D_KEY_HELD;
    }
    if is_key_pressed(KeyCode::D) {
        result |= D_KEY_PRESSED;
    }

    if is_key_down(KeyCode::Up) {
        result |= UP_KEY_HELD;
    }
    if is_key_pressed(KeyCode::Up) {
        result |= UP_KEY_PRESSED;
    }
    if is_key_down(KeyCode::Left) {
        result |= LEFT_KEY_HELD;
    }
    if is_key_pressed(KeyCode::Left) {
        result |= LEFT_KEY_PRESSED;
    }
    if is_key_down(KeyCode::Down) {
        result |= DOWN_KEY_HELD;
    }
    if is_key_pressed(KeyCode::Down) {
        result |= DOWN_KEY_PRESSED;
    }
    if is_key_down(KeyCode::Right) {
        result |= RIGHT_KEY_HELD;
    }
    if is_key_pressed(KeyCode::Right) {
        result |= RIGHT_KEY_PRESSED;
    }

    if is_key_pressed(KeyCode::Space) {
        result |= SPACE_KEY_PRESSED;
    }

    if is_key_down(KeyCode::Space) {
        result |= SPACE_KEY_HELD;
    }

    result
}
