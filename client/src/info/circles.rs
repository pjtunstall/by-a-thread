use std::f32::consts::PI;

use macroquad::prelude::*;

use super::BG_COLOR;
use crate::frame::FrameRate;
use common::player::PlayerState;

pub fn draw_compass(local_state: &PlayerState, x: f32, y: f32, radius: f32) {
    let r = radius;
    draw_circle(x, y, r, BG_COLOR);

    let theta = local_state.yaw + PI;
    let c = vec2(x, y);
    let cos = theta.cos() * r;
    let sin = theta.sin() * r;
    let front = vec2(-sin, cos);
    let side = vec2(cos, sin) * 0.2;
    draw_triangle(c + side, c - side, c - front, BLACK);
    draw_triangle(c + side, c - side, c + front, RED);
}

pub fn draw_fps(fps: &FrameRate, x: f32, y: f32, radius: f32, font: &Font, font_size: u16) {
    draw_circle(x, y, radius, BG_COLOR);

    let font = Some(font);
    let text = format!("{:.0}", fps.rate);
    let text_dims = measure_text(&text, font, font_size, 1.0);

    let text_x = x - text_dims.width / 2.0; // Left edge of text.
    let text_y = y + text_dims.height / 2.0; // Base of text.

    draw_text_ex(
        &text,
        text_x,
        text_y,
        TextParams {
            font,
            font_size,
            color: BLACK,
            ..Default::default()
        },
    );
}

pub fn draw_health(
    health: u8,
    max_health: u8,
    x: f32,
    y: f32,
    radius: f32,
    font: &Font,
    font_size: u16,
) {
    let rim = radius * 0.22;
    let max = max_health.max(1) as f32;
    let health_ratio = (health as f32 / max).clamp(0.0, 1.0);

    let speed_factor = (1.0 - health_ratio) * 10.0;
    let root = ((get_time() as f32 * speed_factor) % (std::f32::consts::PI * 2.0)).sin();
    let a = if health_ratio > 0.6 || health == 0 {
        1.0
    } else {
        root * root
    };

    let red = Color::new(1.0, 0.0, 0.0, a);
    let green = Color::new(0.0, 1.0, 0.0, a);

    draw_circle(x, y, radius, BG_COLOR);

    let start_angle = 270.0;
    let sweep = 360.0 * health_ratio;

    draw_arc(
        x,
        y,
        32,
        radius - rim,
        start_angle + sweep,
        rim,
        360.0 * (1.0 - health_ratio),
        red,
    );
    draw_arc(x, y, 32, radius - rim, start_angle, rim, sweep, green);

    let font = Some(font);
    let text = format!("{}", health);
    let text_dims = measure_text(&text, font, font_size, 1.0);

    let text_x = x - text_dims.width / 2.0; // Left edge of text.
    let text_y = y + text_dims.height / 2.0 - radius * 0.055; // Base of text.

    draw_text_ex(
        &text,
        text_x,
        text_y,
        TextParams {
            font,
            font_size,
            color: BLACK,
            ..Default::default()
        },
    );
}

pub fn draw_timer(estimated_server_time: f64, start_time: f64, x: f32, y: f32, radius: f32) {
    draw_circle(x, y, radius, BG_COLOR);

    let elapsed_time = estimated_server_time - start_time;
    let minutes_elapsed = elapsed_time / 60.0;
    let rim_progress = (minutes_elapsed / 3.0).clamp(0.0, 1.0) as f32; // The rim will fills with red over 3 minutes.

    let phase = (elapsed_time * elapsed_time) / 36.0;
    let root = phase.sin() as f32;
    let a = if rim_progress < 0.4 || rim_progress == 0.0 {
        1.0
    } else {
        root * root
    };

    let rim = radius * 0.22;
    let red = Color::new(1.0, 0.0, 0.0, a);
    let green = Color::new(0.0, 1.0, 0.0, a);

    if rim_progress > 0.0 {
        let start_angle = 270.0;
        let sweep = 360.0 * rim_progress;
        draw_arc(
            x,
            y,
            32,
            radius - rim,
            start_angle + sweep,
            rim,
            360.0 - sweep,
            green,
        );
        draw_arc(x, y, 32, radius - rim, start_angle, rim, sweep, red);
    }

    let seconds_in_minute = elapsed_time % 60.0;
    let timer_progress = (seconds_in_minute / 60.0) as f32;
    let hand_angle = timer_progress * 2.0 * PI - PI / 2.0;
    let center = vec2(x, y);
    let cos = hand_angle.cos() * radius * 0.8;
    let sin = hand_angle.sin() * radius * 0.8;
    let tip = vec2(cos, sin);
    let side = vec2(-sin, cos) * 0.15;

    draw_triangle(center + side, center - side, center + tip, BLACK);

    for i in 0..12 {
        let marker_angle = (i as f32 * 30.0).to_radians() - PI / 2.0;
        let inner_radius = radius * 0.75;
        let outer_radius = radius * 0.92;

        let inner_x = x + marker_angle.cos() * inner_radius;
        let inner_y = y + marker_angle.sin() * inner_radius;
        let outer_x = x + marker_angle.cos() * outer_radius;
        let outer_y = y + marker_angle.sin() * outer_radius;

        let dx = outer_x - inner_x;
        let dy = outer_y - inner_y;
        let length = (dx * dx + dy * dy).sqrt();
        if length > 0.0 {
            let circles = 8;
            for j in 0..circles {
                let t = j as f32 / (circles - 1) as f32;
                let circle_x = inner_x + dx * t;
                let circle_y = inner_y + dy * t;
                draw_circle(circle_x, circle_y, 1.0, BLACK);
            }
        }
    }
}
