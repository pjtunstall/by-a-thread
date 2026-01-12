use macroquad::prelude::*;

use super::BG_COLOR;
use crate::{
    frame::FrameRate,
    local_player::{LocalPlayer, MAX_HEALTH},
};

const TOP_CIRCLE_Y: f32 = 39.0; // Center of top info circle.
const GAP: f32 = 48.0; // Vertical gap between info circles.
const INFO_CIRCLE_RADIUS: f32 = 18.0;
const STAT_NUM_FONT_SIZE: u16 = 16;

pub fn draw_compass(local_player: &LocalPlayer, x: f32) {
    let r = INFO_CIRCLE_RADIUS;
    let y = TOP_CIRCLE_Y;
    draw_circle(x, y, r, BG_COLOR);

    let theta = local_player.yaw;
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

pub fn draw_health(health: u8, x: f32, font: &Font) {
    let rim = 4.0;
    let y = TOP_CIRCLE_Y + GAP * 2.0;

    let max = MAX_HEALTH as f32;
    let health_ratio = health as f32 / max;

    let speed_factor = (1.0 - health_ratio) * 10.0;
    let root = ((get_time() as f32 * speed_factor) % (std::f32::consts::PI * 2.0)).sin();
    let a = if health_ratio > 0.6 || health == 0 {
        1.0
    } else {
        root * root
    };

    let red = Color::new(1.0, 0.0, 0.0, a);
    let green = Color::new(0.0, 1.0, 0.0, a);

    draw_circle(x, y, INFO_CIRCLE_RADIUS, BG_COLOR);

    let start_angle = 270.0;
    let sweep = 360.0 * health_ratio;

    draw_arc(
        x,
        y,
        32,
        INFO_CIRCLE_RADIUS - rim,
        start_angle,
        rim,
        sweep,
        green,
    );

    draw_arc(
        x,
        y,
        32,
        INFO_CIRCLE_RADIUS - rim,
        start_angle + sweep,
        rim,
        360.0 - sweep,
        red,
    );

    let font = Some(font);
    let text = format!("{}", health);
    let text_dims = measure_text(&text, font, STAT_NUM_FONT_SIZE, 1.0);

    let text_x = x - text_dims.width / 2.0; // Left edge of text.
    let text_y = y + text_dims.height / 2.0 - 1.0; // Base of text.

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
