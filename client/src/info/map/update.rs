use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::game::state::Game;
// Color aliases: https://docs.rs/macroquad/latest/macroquad/color/colors/index.html
pub const COLORS: [Color; 10] = [
    RED, LIME, PINK, YELLOW, GREEN, BLUE, MAROON, ORANGE, PURPLE, SKYBLUE,
];
const PLAYER_SYMBOL: &str = "█";

pub fn draw_players_on_map(
    game_state: &Game,
    padding: f32,
    x_indentation: f32,
    y_indentation: f32,
    line_height: f32,
    font: &Font,
    font_size: u16,
) {
    let maze = &game_state.maze;
    let font = Some(font);
    let wall_metrics = measure_text(WALL_SYMBOL, font, font_size, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, font, font_size, 1.0);

    let symbol_width = wall_metrics.width.max(space_metrics.width);

    let mut color_index = 0;
    let local_index = game_state.local_player_index;
    let local_player = &game_state.players[local_index];

    if let Some((col, row)) = maze.grid_coordinates_from_position(&local_player.state.position) {
        draw_text_ex(
            PLAYER_SYMBOL,
            x_indentation + padding + (col as f32) * symbol_width,
            y_indentation + padding + (row as f32 + 1.0) * line_height,
            TextParams {
                font,
                font_size,
                color: COLORS[color_index % COLORS.len()],
                ..Default::default()
            },
        );
    }

    for (index, player) in game_state.players.iter().enumerate() {
        if index == local_index || player.health == 0 {
            continue;
        }

        color_index += 1;
        if let Some((col, row)) = maze.grid_coordinates_from_position(&player.state.position) {
            draw_text_ex(
                PLAYER_SYMBOL,
                x_indentation + padding + (col as f32) * symbol_width,
                y_indentation + padding + (row as f32 + 1.0) * line_height,
                TextParams {
                    font,
                    font_size,
                    color: COLORS[color_index % COLORS.len()],
                    ..Default::default()
                },
            );
        }
    }
}
