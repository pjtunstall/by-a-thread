use macroquad::prelude::*;

use crate::lobby::ui::LobbyUi;

pub fn should_quit() -> bool {
    is_quit_requested() || is_key_pressed(KeyCode::Escape)
}

pub async fn wait_till_escape_is_pressed(
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) {
    ui.show_warning("Press ESCAPE to exit.");
    while !should_quit() {
        ui.draw(false, false, font, None);
        next_frame().await;
    }
}
