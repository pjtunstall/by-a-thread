use macroquad::prelude::*;

pub fn should_quit() -> bool {
    is_quit_requested() || is_key_pressed(KeyCode::Escape)
}
