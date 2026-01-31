use std::fmt;

use glam::Vec3;
use macroquad::prelude::*;

use crate::{
    assets::Assets,
    info::{self, map::MapOverlay},
};
use common::{maze::Maze, player::Color};

const AFTER_GAME_MAP_BORDER_THICKNESS: f32 = 16.0;
const AFTER_GAME_MAP_BORDER_ALPHA: f32 = 0.5;
const AFTER_GAME_MAP_SCALE: f32 = 1.2; // Compared to in-game map.

pub struct AfterGameMap {
    pub map_overlay: MapOverlay,
    pub maze: Maze,
    pub positions: Vec<(Vec3, Color)>,
}

impl fmt::Debug for AfterGameMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfterGameMap")
            .field("maze", &self.maze)
            .field("positions", &self.positions)
            .finish_non_exhaustive()
    }
}

pub fn draw_after_game_map(data: &AfterGameMap, assets: &Assets) {
    push_camera_state();
    set_default_camera();
    let rect_h = data.map_overlay.rect.h;
    let map_scale =
        AFTER_GAME_MAP_SCALE * screen_height() * info::MAP_FRACTION_OF_SCREEN_HEIGHT / rect_h;
    let map_w = data.map_overlay.rect.w * map_scale;
    let map_h = rect_h * map_scale;
    let margin = info::BASE_INDENTATION;
    let border_w = map_w + 2.0 * AFTER_GAME_MAP_BORDER_THICKNESS;
    let border_h = map_h + 2.0 * AFTER_GAME_MAP_BORDER_THICKNESS;
    let border_x = screen_width() - margin - border_w;
    let border_y = margin;
    draw_rectangle(
        border_x,
        border_y,
        border_w,
        border_h,
        macroquad::prelude::Color::new(0.0, 0.0, 0.0, AFTER_GAME_MAP_BORDER_ALPHA),
    );
    let map_x = screen_width() - margin - AFTER_GAME_MAP_BORDER_THICKNESS - map_w;
    let map_y = margin + AFTER_GAME_MAP_BORDER_THICKNESS;
    info::draw_map_at(
        map_x,
        map_y,
        &data.map_overlay,
        &data.maze,
        &data.positions,
        assets,
        map_scale,
    );
    pop_camera_state();
}
