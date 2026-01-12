use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::{info::FONT_SIZE, maze::maker::Cell, session::Session};

// Color aliases: https://docs.rs/macroquad/latest/macroquad/color/colors/index.html
pub const COLORS: [Color; 10] = [
    RED, LIME, PINK, YELLOW, GREEN, BLUE, MAROON, ORANGE, PURPLE, SKYBLUE,
];
const PLAYER_SYMBOL: &str = "█";

pub fn update_players_on_map(session: &mut Session) {
    let Cell { x, y } = session.local_player.get_cell();
    session.x_positions_on_map[0] = x as u8;
    session.y_positions_on_map[0] = y as u8;

    session
        .remote_players
        .iter()
        .enumerate()
        .for_each(|(i, b)| {
            if !b.is_alive {
                return;
            }
            let Cell { x, y } = b.get_cell();
            session.x_positions_on_map[i + 1] = x as u8;
            session.y_positions_on_map[i + 1] = y as u8;
        });
}

pub fn draw_players_on_map(
    session: &Session,
    padding: f32,
    x_indentation: f32,
    y_indentation: f32,
    line_height: f32,
) {
    let Session {
        x_positions_on_map,
        y_positions_on_map,
        remote_players,
        font,
        ..
    } = session;

    let wall_metrics = measure_text(WALL_SYMBOL, Some(font), FONT_SIZE as u16, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, Some(font), FONT_SIZE as u16, 1.0);

    let symbol_width = wall_metrics.width.max(space_metrics.width);

    x_positions_on_map
        .iter()
        .enumerate()
        .zip(y_positions_on_map.iter())
        .for_each(|((i, &col), &row)| {
            if i == 0 || remote_players[i - 1].is_alive {
                draw_text_ex(
                    PLAYER_SYMBOL,
                    x_indentation + padding + (col as f32) * symbol_width,
                    y_indentation + padding + (row as f32 + 1.0) * line_height,
                    TextParams {
                        font: Some(font),
                        font_size: FONT_SIZE as u16,
                        color: COLORS[i],
                        ..Default::default()
                    },
                );
            }
        });
}
