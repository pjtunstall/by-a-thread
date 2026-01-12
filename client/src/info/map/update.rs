use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::{game::state::Game, info::FONT_SIZE};
use common::maze::CELL_SIZE;

// Color aliases: https://docs.rs/macroquad/latest/macroquad/color/colors/index.html
pub const COLORS: [Color; 10] = [
    RED, LIME, PINK, YELLOW, GREEN, BLUE, MAROON, ORANGE, PURPLE, SKYBLUE,
];
const PLAYER_SYMBOL: &str = "█";

fn map_coordinates(grid: &[Vec<u8>], position: Vec3) -> Option<(u8, u8)> {
    let col = (position.x / CELL_SIZE).floor() as isize;
    let row = (position.z / CELL_SIZE).floor() as isize;

    if col < 0 || row < 0 {
        return None;
    }

    let col = col as usize;
    let row = row as usize;

    if row >= grid.len() || col >= grid[0].len() {
        return None;
    }

    Some((col as u8, row as u8))
}

pub fn draw_players_on_map(
    game_state: &Game,
    padding: f32,
    x_indentation: f32,
    y_indentation: f32,
    line_height: f32,
    font: &Font,
) {
    let font = Some(font);
    let wall_metrics = measure_text(WALL_SYMBOL, font, FONT_SIZE as u16, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, font, FONT_SIZE as u16, 1.0);

    let symbol_width = wall_metrics.width.max(space_metrics.width);
    let grid = &game_state.maze.grid;

    let mut color_index = 0;
    let local_index = game_state.local_player_index;
    let local_player = &game_state.players[local_index];

    if let Some((col, row)) = map_coordinates(grid, local_player.state.position) {
        draw_text_ex(
            PLAYER_SYMBOL,
            x_indentation + padding + (col as f32) * symbol_width,
            y_indentation + padding + (row as f32 + 1.0) * line_height,
            TextParams {
                font,
                font_size: FONT_SIZE as u16,
                color: COLORS[color_index % COLORS.len()],
                ..Default::default()
            },
        );
    }

    for (index, player) in game_state.players.iter().enumerate() {
        if index == local_index || !player.alive {
            continue;
        }

        color_index += 1;
        if let Some((col, row)) = map_coordinates(grid, player.state.position) {
            draw_text_ex(
                PLAYER_SYMBOL,
                x_indentation + padding + (col as f32) * symbol_width,
                y_indentation + padding + (row as f32 + 1.0) * line_height,
                TextParams {
                    font,
                    font_size: FONT_SIZE as u16,
                    color: COLORS[color_index % COLORS.len()],
                    ..Default::default()
                },
            );
        }
    }
}
