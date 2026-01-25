use macroquad::prelude::*;
use std::f32::consts::PI;

use super::BG_COLOR;
use crate::frame::FrameRate;
use common::player::PlayerState;

pub struct TimerMarkers {
    pub render_target: RenderTarget,
    pub radius: f32,
}

impl TimerMarkers {
    pub fn new(radius: f32) -> Self {
        let render_target = Self::create_marker_render_target(radius);
        Self {
            render_target,
            radius,
        }
    }

    fn create_marker_render_target(radius: f32) -> RenderTarget {
        const SUPERSAMPLE: f32 = 4.0;

        let logical_size = radius * 1.84;

        let texture_size = (logical_size * SUPERSAMPLE).ceil() as u32;
        let w = texture_size as f32;
        let h = texture_size as f32;
        let center = w / 2.0;

        let scaled_radius = radius * SUPERSAMPLE;
        let marker_thickness = 2.0 * SUPERSAMPLE;

        let render_target = render_target(texture_size, texture_size);
        render_target.texture.set_filter(FilterMode::Linear);

        let mut camera = Camera2D {
            render_target: Some(render_target),
            zoom: vec2(2.0 / w, 2.0 / h),
            target: vec2(w / 2.0, h / 2.0),
            ..Default::default()
        };
        camera.zoom.y *= -1.0;

        set_camera(&camera);

        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        for i in 0..12 {
            let marker_angle = (i as f32 * 30.0).to_radians() - PI / 2.0;

            let inner_radius = scaled_radius * 0.75;
            let outer_radius = scaled_radius * 0.92;

            let inner_x = center + marker_angle.cos() * inner_radius;
            let inner_y = center + marker_angle.sin() * inner_radius;
            let outer_x = center + marker_angle.cos() * outer_radius;
            let outer_y = center + marker_angle.sin() * outer_radius;

            let dx = outer_x - inner_x;
            let dy = outer_y - inner_y;
            let length = (dx * dx + dy * dy).sqrt();

            if length > 0.0 {
                draw_line(inner_x, inner_y, outer_x, outer_y, marker_thickness, BLACK);

                let cap_radius = marker_thickness / 2.0;
                draw_circle(inner_x, inner_y, cap_radius, BLACK);
                draw_circle(outer_x, outer_y, cap_radius, BLACK);
            }
        }

        set_default_camera();

        camera.render_target.take().unwrap()
    }
}

const HEALTH_FLASH_START_THRESHOLD: f32 = 0.6;
const TIMER_FLASH_START_THRESHOLD: f32 = 0.9;
const MIN_FLASH_SPEED: f32 = 4.0;
const MAX_FLASH_SPEED: f32 = 10.0;

pub fn draw_compass(local_state: &PlayerState, x: f32, y: f32, radius: f32) {
    let r = radius;
    draw_circle(x, y, r, BG_COLOR);

    let theta = local_state.yaw + PI;
    let center = vec2(x, y);
    let cos = theta.cos() * r;
    let sin = theta.sin() * r;
    let front = vec2(-sin, cos);
    let side = vec2(cos, sin) * 0.2;
    draw_triangle(center + side, center - side, center - front, BLACK);
    draw_triangle(center + side, center - side, center + front, RED);
}

pub fn draw_fps(fps: &FrameRate, x: f32, y: f32, radius: f32, font: &Font, font_size: u16) {
    draw_circle(x, y, radius, BG_COLOR);

    let font = Some(font);
    let text = format!("{:.0}", fps.rate);
    let text_dims = measure_text(&text, font, font_size, 1.0);

    let text_x = x - text_dims.width / 2.0;
    let text_y = y + text_dims.height / 2.0;

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
    let max = max_health.max(1) as f32;
    let health_ratio = (health as f32 / max).clamp(0.0, 1.0);

    let severity = 1.0 - health_ratio;

    let (current_speed, should_flash) = get_flash_params(severity, HEALTH_FLASH_START_THRESHOLD);

    let phase = get_time() as f32 * current_speed;

    let flash_opacity = calculate_flash_opacity(phase, should_flash);

    let danger_color = Color::new(1.0, 0.0, 0.0, 1.0);
    let safety_color = Color::new(0.0, 1.0, 0.0, 1.0);

    draw_severity_arcs(
        x,
        y,
        radius,
        danger_color,
        safety_color,
        severity,
        flash_opacity,
    );

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

pub fn draw_timer(
    estimated_server_time: f64,
    start_time: f64,
    x: f32,
    y: f32,
    radius: f32,
    markers: &TimerMarkers,
) {
    let total_duration = 180.0;
    let elapsed_time = (estimated_server_time - start_time) as f32;
    let minutes_elapsed = elapsed_time / 60.0;

    let rim_progress = (minutes_elapsed / 3.0).clamp(0.0, 1.0);
    let severity = rim_progress;

    let (current_speed, should_flash) = get_flash_params(severity, TIMER_FLASH_START_THRESHOLD);

    let time_flash_started = total_duration * TIMER_FLASH_START_THRESHOLD;
    let time_in_zone = (elapsed_time - time_flash_started).max(0.0);

    let average_speed = (MIN_FLASH_SPEED + current_speed) / 2.0;
    let phase = time_in_zone * average_speed;

    let flash_opacity = calculate_flash_opacity(phase, should_flash);

    let danger_color = Color::new(1.0, 0.0, 0.0, 1.0);
    let safety_color = Color::new(0.0, 1.0, 0.0, 1.0);

    draw_severity_arcs(
        x,
        y,
        radius,
        danger_color,
        safety_color,
        severity,
        flash_opacity,
    );

    let seconds_in_minute = elapsed_time % 60.0;
    let timer_progress = seconds_in_minute / 60.0;
    let hand_angle = timer_progress * 2.0 * PI - PI / 2.0;
    let center = vec2(x, y);
    let cos = hand_angle.cos() * radius * 0.8;
    let sin = hand_angle.sin() * radius * 0.8;
    let tip = vec2(cos, sin);
    let side = vec2(-sin, cos) * 0.15;

    draw_triangle(center + side, center - side, center + tip, BLACK);

    let texture_size_logical = markers.radius * 1.84;
    let scale = radius / markers.radius;
    let scaled_texture_size = texture_size_logical * scale;

    let dest_pos = vec2(x - scaled_texture_size / 2.0, y - scaled_texture_size / 2.0);

    draw_texture_ex(
        &markers.render_target.texture,
        dest_pos.x,
        dest_pos.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(scaled_texture_size, scaled_texture_size)),
            ..Default::default()
        },
    );
}

fn get_flash_params(severity: f32, flash_start_threshold: f32) -> (f32, bool) {
    if severity < flash_start_threshold {
        return (0.0, false);
    }

    let danger_progress = (severity - flash_start_threshold) / (1.0 - flash_start_threshold);
    let clamped_danger = danger_progress.clamp(0.0, 1.0);

    let speed = MIN_FLASH_SPEED + (MAX_FLASH_SPEED - MIN_FLASH_SPEED) * clamped_danger;

    (speed, true)
}

fn calculate_flash_opacity(phase: f32, should_flash: bool) -> f32 {
    if !should_flash {
        return 1.0;
    }
    let root = (phase % (PI * 2.0)).sin();
    root * root
}

fn draw_severity_arcs(
    x: f32,
    y: f32,
    radius: f32,
    mut danger_color: Color,
    mut safety_color: Color,
    severity: f32,
    flash_opacity: f32,
) {
    draw_circle(x, y, radius, BG_COLOR);

    let rim = radius * 0.22;
    danger_color.a = flash_opacity;
    safety_color.a = flash_opacity;

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
