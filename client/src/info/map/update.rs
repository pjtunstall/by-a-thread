use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use common::maze::Maze;
use common::player::Color as PlayerColor;
use glam::Vec3;

const PLAYER_SYMBOL: &str = "█";

fn player_color_to_macroquad_color(color: PlayerColor) -> Color {
    match color {
        PlayerColor::RED => RED,
        PlayerColor::LIME => LIME,
        PlayerColor::PINK => PINK,
        PlayerColor::YELLOW => YELLOW,
        PlayerColor::GREEN => GREEN,
        PlayerColor::BLUE => BLUE,
        PlayerColor::MAROON => MAROON,
        PlayerColor::ORANGE => ORANGE,
        PlayerColor::PURPLE => PURPLE,
        PlayerColor::SKYBLUE => SKYBLUE,
    }
}

pub fn draw_player_positions_on_map(
    maze: &Maze,
    positions: &[(Vec3, PlayerColor)],
    base_x: f32,
    base_y: f32,
    padding: f32,
    line_height: f32,
    map_font: &Font,
    font_size: u16,
) {
    let wall_metrics = measure_text(WALL_SYMBOL, Some(map_font), font_size, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, Some(map_font), font_size, 1.0);
    let symbol_width = wall_metrics.width.max(space_metrics.width);

    for (position, color) in positions {
        if let Some((col, row)) = maze.grid_coordinates_from_position(position) {
            draw_text_ex(
                PLAYER_SYMBOL,
                base_x + padding + (col as f32) * symbol_width,
                base_y + padding + (row as f32 + 1.0) * line_height,
                TextParams {
                    font: Some(map_font),
                    font_size,
                    color: player_color_to_macroquad_color(*color),
                    ..Default::default()
                },
            );
        }
    }
}
