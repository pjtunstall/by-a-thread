use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::info::{BG_COLOR, FONT_SIZE};
use common::maze::{Maze, RADIUS};

pub struct MapOverlay {
    pub texture: Texture2D,
    pub rect: Rect,
}

// A map of the maze is drawn as text and captured as a texture to avoid having
// to calculate where all the characters for spaces and walls should be printed
// on every frame.
pub fn initialize_map(maze: &Maze, font: &Font) -> MapOverlay {
    clear_background(BG_COLOR);
    let map_string = create_map_string(&maze.grid);
    let rect = draw_initial_map(&map_string, font);
    let texture = create_map_texture(rect);

    MapOverlay { texture, rect }
}

fn create_map_texture(rect: Rect) -> Texture2D {
    let screen = get_screen_data();
    let map_image = screen.sub_image(rect);
    let map_texture = Texture2D::from_image(&map_image);
    map_texture.set_filter(FilterMode::Linear);
    map_texture
}

fn create_map_string(grid: &[Vec<u8>]) -> String {
    let mut map_string = String::new();

    for row in grid {
        for &cell in row {
            match cell {
                0 => map_string.push_str(SPACE_SYMBOL),
                _ => map_string.push_str(WALL_SYMBOL),
            }
        }
        map_string.push('\n');
    }

    map_string
}

fn draw_initial_map(map: &str, font: &Font) -> Rect {
    push_camera_state();
    set_default_camera();

    let padding = 10.0;
    let x_indentation = 10.0;
    let y_indentation = 10.0;

    let line_height = FONT_SIZE;

    let wall_metrics = measure_text(WALL_SYMBOL, Some(font), FONT_SIZE as u16, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, Some(font), FONT_SIZE as u16, 1.0);

    let symbol_width = wall_metrics.width.max(space_metrics.width);

    let total_width = ((RADIUS * 2 + 1) as f32) * symbol_width;
    let total_height = (RADIUS * 2 + 1) as f32 * line_height;

    let w = total_width + x_indentation * 2.2;
    let h = total_height + y_indentation * 2.2;

    draw_rectangle(x_indentation, y_indentation, w, h, BG_COLOR);

    for (row_idx, line) in map.lines().enumerate() {
        let mut x_pos = x_indentation + padding;
        let y_pos = y_indentation + padding + (row_idx as f32 + 1.0) * line_height;

        for ch in line.chars() {
            draw_text_ex(
                &ch.to_string(),
                x_pos,
                y_pos,
                TextParams {
                    font: Some(font),
                    font_size: FONT_SIZE as u16,
                    color: BLACK,
                    ..Default::default()
                },
            );
            x_pos += symbol_width;
        }
    }

    pop_camera_state();

    Rect::new(x_indentation, screen_height() - y_indentation - h, w, h)
}
