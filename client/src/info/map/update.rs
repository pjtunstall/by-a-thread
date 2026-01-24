use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::game::state::Game;
use common::player::Color as PlayerColor;

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

pub fn draw_players_on_map(
    game_state: &Game,
    padding: f32,
    x_indentation: f32,
    y_indentation: f32,
    line_height: f32,
    map_font: &Font,
    font_size: u16,
) {
    let maze = &game_state.maze;
    let wall_metrics = measure_text(WALL_SYMBOL, Some(map_font), font_size, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, Some(map_font), font_size, 1.0);
    let symbol_width = wall_metrics.width.max(space_metrics.width);

    for player in game_state.players.iter() {
        if !player.is_alive() {
            continue;
        }

        if let Some((col, row)) = maze.grid_coordinates_from_position(&player.state.position) {
            draw_text_ex(
                PLAYER_SYMBOL,
                x_indentation + padding + (col as f32) * symbol_width,
                y_indentation + padding + (row as f32 + 1.0) * line_height,
                TextParams {
                    font: Some(map_font),
                    font_size,
                    color: player_color_to_macroquad_color(player.color),
                    ..Default::default()
                },
            );
        }
    }
}
