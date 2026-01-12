mod circles;
pub mod map;

use macroquad::prelude::*;

use crate::session::ClientSession;

pub const FONT_SIZE: f32 = 6.0;
pub const BG_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.8);

pub fn update(session: &mut ClientSession) {
    map::update::update_players_on_map(session);
}

pub fn draw(session: &ClientSession) {
    let ClientSession {
        map,
        local_player,
        fps,
        map_rect,
        font,
        ..
    } = session;

    push_camera_state();
    set_default_camera();

    let padding = 10.0;
    let x_indentation = 10.0;
    let y_indentation = 10.0;
    let line_height = FONT_SIZE;

    // Draw map.
    draw_texture_ex(
        map,
        x_indentation,
        y_indentation,
        WHITE,
        DrawTextureParams {
            flip_y: true,
            ..Default::default()
        },
    );

    map::update::draw_players_on_map(session, padding, x_indentation, y_indentation, line_height);

    let x = map_rect
        .expect("map rect should exist by now; see `main`")
        .w
        + 40.0;
    circles::draw_compass(local_player, x);
    circles::draw_fps(fps, x, font);
    circles::draw_health(local_player.health, x, font);

    pop_camera_state();
}
