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
    let start_flashing_at = 0.6;
    let min_speed = 4.0;
    let max_speed = 10.0;

    draw_circle(x, y, radius, BG_COLOR);

    let health_ratio = (health as f32 / max_health as f32).clamp(0.0, 1.0);
    let severity = 1.0 - health_ratio;

    let a = calculate_flash_alpha(
        severity,
        start_flashing_at,
        FlashSpeed {
            min: min_speed,
            max: max_speed,
        },
        get_time() as f32,
    );

    let danger_color = Color::new(1.0, 0.0, 0.0, a);
    let safety_color = Color::new(0.0, 1.0, 0.0, a);
    draw_severity(x, y, radius, danger_color, safety_color, severity);

    let font = Some(font);
    let text = format!("{}", health);
    let text_dims = measure_text(&text, font, font_size, 1.0);

    let text_x = x - text_dims.width / 2.0;
    let text_y = y + text_dims.height / 2.0 - radius * 0.055;

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
    let start_flashing_at = 0.9;
    let min_speed = 4.0;
    let max_speed = 10.0;

    draw_circle(x, y, radius, BG_COLOR);

    let total_duration = 180.0;
    let elapsed_time = (estimated_server_time - start_time) as f32;
    let minutes_elapsed = elapsed_time / 60.0;
    let severity = (minutes_elapsed / 3.0).clamp(0.0, 1.0);

    let time_flash_started = total_duration * start_flashing_at;
    let time_in_zone = (elapsed_time - time_flash_started).max(0.0);

    let a = calculate_flash_alpha(
        severity,
        start_flashing_at,
        FlashSpeed {
            min: min_speed,
            max: max_speed,
        },
        time_in_zone,
    );

    let danger_color = Color::new(1.0, 0.0, 0.0, a);
    let safety_color = Color::new(0.0, 1.0, 0.0, a);
    draw_severity(x, y, radius, danger_color, safety_color, severity);

    let seconds_in_minute = elapsed_time % 60.0;
    let timer_progress = seconds_in_minute / 60.0;
    let hand_angle = timer_progress * 2.0 * PI - PI / 2.0;
    let center = vec2(x, y);
    let cos = hand_angle.cos() * radius * 0.8;
    let sin = hand_angle.sin() * radius * 0.8;
    let tip = vec2(cos, sin);
    let side = vec2(-sin, cos) * 0.15;

    draw_triangle(center + side, center - side, center + tip, BLACK);

    // TODO: Create texture initially to save redrawing every frame.
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

struct FlashSpeed {
    min: f32,
    max: f32,
}

fn calculate_flash_alpha(
    severity: f32,
    start_flashing_at: f32,
    speed: FlashSpeed,
    time_in_zone: f32,
) -> f32 {
    let danger_progress = (severity - start_flashing_at) / (1.0 - start_flashing_at);
    let clamped_danger = danger_progress.clamp(0.0, 1.0);

    let current_speed = speed.min + (speed.max - speed.min) * clamped_danger;
    let phase = time_in_zone * current_speed;

    let root = (phase % (PI * 2.0)).sin();
    if severity < start_flashing_at || severity > 0.999 {
        1.0
    } else {
        root * root
    }
}

fn draw_severity(
    x: f32,
    y: f32,
    radius: f32,
    danger_color: Color,
    safety_color: Color,
    severity: f32,
) {
    let rim = radius * 0.22;
    let start_angle = 270.0;
    let sweep = 360.0 * severity;

    draw_arc(
        x,
        y,
        32,
        radius - rim,
        start_angle + sweep,
        rim,
        360.0 - sweep,
        safety_color,
    );
    draw_arc(
        x,
        y,
        32,
        radius - rim,
        start_angle,
        rim,
        sweep,
        danger_color,
    );
}
