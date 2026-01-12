mod circles;
pub mod map;

use macroquad::prelude::*;

use crate::{assets::Assets, frame::FrameRate, game::state::Game};

pub const FONT_SIZE: f32 = 6.0;
pub const BG_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.8);

pub fn draw(game_state: &Game, assets: &Assets, fps: &FrameRate) {
    let local_player = &game_state.players[game_state.local_player_index];
    let local_state = &local_player.state;

    push_camera_state();
    set_default_camera();

    let padding = 10.0;
    let x_indentation = 10.0;
    let y_indentation = 10.0;
    let line_height = FONT_SIZE;

    // Draw map.
    let map_overlay = &game_state.info_map;

    draw_texture_ex(
        &map_overlay.texture,
        x_indentation,
        y_indentation,
        WHITE,
        DrawTextureParams {
            flip_y: true,
            ..Default::default()
        },
    );

    map::update::draw_players_on_map(
        game_state,
        padding,
        x_indentation,
        y_indentation,
        line_height,
        &assets.font,
    );

    let x = map_overlay.rect.w + 40.0;
    circles::draw_compass(local_state, x);
    circles::draw_fps(fps, x, &assets.font);

    pop_camera_state();
}
