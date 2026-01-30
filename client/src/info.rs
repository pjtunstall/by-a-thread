pub mod circles;
mod crosshairs;
pub mod map;

use glam::Vec3;
use macroquad::prelude::*;

use crate::{assets::Assets, frame::FrameRate, game::state::Game};
use common::{maze::Maze, player::Color as PlayerColor, player::MAX_HEALTH};

pub const FONT_SIZE: f32 = 6.0;
pub const MAP_FRACTION_OF_SCREEN_HEIGHT: f32 = 0.5;
pub const BG_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.8);
pub const BASE_CIRCLE_RADIUS: f32 = 18.0;
pub const BASE_INDENTATION: f32 = 10.0;
pub const BASE_PADDING: f32 = 10.0;
const BASE_CIRCLE_TOP_OFFSET: f32 = 29.0; // Top circle center minus indentation.
const BASE_CIRCLE_GAP: f32 = 48.0;
const BASE_STAT_FONT_SIZE: u16 = 16;
const BASE_MAP_TO_STATS_GAP: f32 = 40.0;

pub fn draw_map_at(
    base_x: f32,
    base_y: f32,
    map_overlay: &map::MapOverlay,
    maze: &Maze,
    positions: &[(Vec3, PlayerColor)],
    assets: &Assets,
    map_scale: f32,
) {
    let padding = BASE_PADDING * map_scale;
    let line_height = FONT_SIZE * map_scale;
    let symbol_width_base =
        map::update::cell_width_at_font_size(&assets.map_font, FONT_SIZE as u16);
    let symbol_width = symbol_width_base * map_scale;
    let font_size = (FONT_SIZE * map_scale).round().max(1.0) as u16;
    draw_texture_ex(
        &map_overlay.render_target.texture,
        base_x,
        base_y,
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
    map::update::draw_player_positions_on_map(
        maze,
        positions,
        base_x,
        base_y,
        padding,
        symbol_width,
        line_height,
        &assets.map_font,
        font_size,
    );
}

pub fn draw(game_state: &Game, assets: &Assets, fps: &FrameRate, estimated_server_time: f64) {
    let local_player = &game_state.players[game_state.local_player_index];
    let local_state = &local_player.state;

    push_camera_state();
    set_default_camera();

    let map_overlay = &game_state.map_overlay;
    let map_scale = screen_height() * MAP_FRACTION_OF_SCREEN_HEIGHT / map_overlay.rect.h;
    let x_indentation = BASE_INDENTATION;
    let y_indentation = BASE_INDENTATION;
    let stat_font_size = (BASE_STAT_FONT_SIZE as f32 * map_scale).round().max(1.0) as u16;

    crosshairs::draw_crosshairs();

    let positions: Vec<_> = game_state
        .players
        .iter()
        .filter(|p| p.is_alive())
        .map(|p| (p.state.position, p.color))
        .collect();
    draw_map_at(
        x_indentation,
        y_indentation,
        map_overlay,
        &game_state.maze,
        &positions,
        assets,
        map_scale,
    );

    let x = x_indentation + map_overlay.rect.w * map_scale + BASE_MAP_TO_STATS_GAP * map_scale;
    let circle_radius = BASE_CIRCLE_RADIUS * map_scale;
    let circle_top = y_indentation + BASE_CIRCLE_TOP_OFFSET * map_scale;
    let circle_gap = BASE_CIRCLE_GAP * map_scale;
    circles::draw_compass(
        local_state,
        x,
        circle_top,
        circle_radius,
        &game_state.needle_textures,
    );
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
        game_state.start_time,
        game_state.timer_duration,
        x,
        circle_top + circle_gap * 3.0,
        circle_radius,
        &game_state.timer_markers,
        &game_state.needle_textures,
    );

    pop_camera_state();
}
