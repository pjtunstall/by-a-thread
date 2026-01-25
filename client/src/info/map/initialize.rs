use macroquad::prelude::*;

use super::{SPACE_SYMBOL, WALL_SYMBOL};
use crate::info::{BG_COLOR, FONT_SIZE};
use common::maze::{Maze, RADIUS};

pub struct MapOverlay {
    pub render_target: RenderTarget,
    pub rect: Rect,
}

pub fn initialize_map(maze: &Maze, map_font: &Font) -> MapOverlay {
    let padding = 10.0;
    let x_indentation = 10.0;
    let y_indentation = 10.0;
    let line_height = FONT_SIZE;

    let wall_metrics = measure_text(WALL_SYMBOL, Some(map_font), FONT_SIZE as u16, 1.0);
    let space_metrics = measure_text(SPACE_SYMBOL, Some(map_font), FONT_SIZE as u16, 1.0);
    let symbol_width = wall_metrics.width.max(space_metrics.width);

    let total_width = ((RADIUS * 2 + 1) as f32) * symbol_width;
    let total_height = (RADIUS * 2 + 1) as f32 * line_height;

    let w = total_width + x_indentation * 2.2;
    let h = total_height + y_indentation * 2.2;

    let render_target = render_target(w as u32, h as u32);
    render_target.texture.set_filter(FilterMode::Linear);

    let mut camera = Camera2D {
        render_target: Some(render_target),
        zoom: vec2(2.0 / w, 2.0 / h),
        target: vec2(w / 2.0, h / 2.0),
        ..Default::default()
    };
    camera.zoom.y *= -1.0;

    set_camera(&camera);

    clear_background(BG_COLOR);

    let map_string = create_map_string(&maze.grid);

    for (row_idx, line) in map_string.lines().enumerate() {
        // Start from 0 relative to the texture
        let mut x_pos = padding;
        let y_pos = padding + (row_idx as f32 + 1.0) * line_height;

        for ch in line.chars() {
            draw_text_ex(
                &ch.to_string(),
                x_pos,
                y_pos,
                TextParams {
                    font: Some(map_font),
                    font_size: FONT_SIZE as u16,
                    color: BLACK,
                    ..Default::default()
                },
            );
            x_pos += symbol_width;
        }
    }

    set_default_camera();

    let final_render_target = camera.render_target.take().unwrap();

    let rect = Rect::new(x_indentation, screen_height() - y_indentation - h, w, h);

    MapOverlay {
        render_target: final_render_target,
        rect,
    }
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
