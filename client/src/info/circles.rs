use macroquad::prelude::*;

use crate::frame::FrameRate;
use common::player::PlayerState;
use super::BG_COLOR;

const TOP_CIRCLE_Y: f32 = 39.0; // Center of top info circle.
const GAP: f32 = 48.0; // Vertical gap between info circles.
const INFO_CIRCLE_RADIUS: f32 = 18.0;
const STAT_NUM_FONT_SIZE: u16 = 16;

pub fn draw_compass(local_state: &PlayerState, x: f32) {
    let r = INFO_CIRCLE_RADIUS;
    let y = TOP_CIRCLE_Y;
    draw_circle(x, y, r, BG_COLOR);

    let theta = local_state.yaw;
    let c = vec2(x, y);
    let cos = theta.cos() * r;
    let sin = theta.sin() * r;
    let front = vec2(-sin, cos);
    let side = vec2(cos, sin) * 0.2;
    draw_triangle(c + side, c - side, c - front, BLACK);
    draw_triangle(c + side, c - side, c + front, RED);
}

pub fn draw_fps(fps: &FrameRate, x: f32, font: &Font) {
    let r = INFO_CIRCLE_RADIUS;
    let y = TOP_CIRCLE_Y + GAP;

    draw_circle(x, y, r, BG_COLOR);

    let font = Some(font);
    let text = format!("{:.0}", fps.rate);
    let text_dims = measure_text(&text, font, STAT_NUM_FONT_SIZE, 1.0);

    let text_x = x - text_dims.width / 2.0; // Left edge of text.
    let text_y = y + text_dims.height / 2.0; // Base of text.

    draw_text_ex(
        &text,
        text_x,
        text_y,
        TextParams {
            font,
            font_size: STAT_NUM_FONT_SIZE,
            color: BLACK,
            ..Default::default()
        },
    );
}
