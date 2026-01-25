mod circles;
mod crosshairs;
pub mod map;

use macroquad::prelude::*;

use crate::{assets::Assets, frame::FrameRate, game::state::Game};
use common::player::MAX_HEALTH;

const BASE_INDENTATION: f32 = 10.0;
const BASE_PADDING: f32 = 10.0;
const BASE_CIRCLE_TOP_OFFSET: f32 = 29.0; // Top circle center minus indentation.
const BASE_CIRCLE_GAP: f32 = 48.0;
const BASE_CIRCLE_RADIUS: f32 = 18.0;
const BASE_STAT_FONT_SIZE: u16 = 16;
const BASE_MAP_TO_STATS_GAP: f32 = 40.0;
pub const INFO_SCALE: f32 = 1.3;
pub const FONT_SIZE: f32 = 6.0;
pub const BG_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.8);

pub fn draw(game_state: &Game, assets: &Assets, fps: &FrameRate, estimated_server_time: f64) {
    let local_player = &game_state.players[game_state.local_player_index];
    let local_state = &local_player.state;

    push_camera_state();
    set_default_camera();

    let font_size = (FONT_SIZE * INFO_SCALE).round().max(1.0) as u16;
    let map_scale = font_size as f32 / FONT_SIZE;
    let padding = BASE_PADDING * map_scale;
    let x_indentation = BASE_INDENTATION;
    let y_indentation = BASE_INDENTATION;
    let line_height = font_size as f32;
    let stat_font_size = (BASE_STAT_FONT_SIZE as f32 * map_scale).round().max(1.0) as u16;

    crosshairs::draw_crosshairs();

    let map_overlay = &game_state.info_map;
    draw_texture_ex(
        &map_overlay.texture,
        x_indentation,
        y_indentation,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(
                map_overlay.rect.w * map_scale,
                map_overlay.rect.h * map_scale,
            )),
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
        &assets.map_font,
        font_size,
    );

    let x = x_indentation + map_overlay.rect.w * map_scale + BASE_MAP_TO_STATS_GAP * map_scale;
    let circle_radius = BASE_CIRCLE_RADIUS * map_scale;
    let circle_top = y_indentation + BASE_CIRCLE_TOP_OFFSET * map_scale;
    let circle_gap = BASE_CIRCLE_GAP * map_scale;
    circles::draw_compass(local_state, x, circle_top, circle_radius);
    circles::draw_fps(
        fps,
        x,
        circle_top + circle_gap,
        circle_radius,
        &assets.font,
        stat_font_size,
    );
    circles::draw_health(
        local_player.health,
        MAX_HEALTH,
        x,
        circle_top + circle_gap * 2.0,
        circle_radius,
        &assets.font,
        stat_font_size,
    );
    circles::draw_timer(
        estimated_server_time,
        x,
        circle_top + circle_gap * 3.0,
        circle_radius,
    );

    pop_camera_state();
}
