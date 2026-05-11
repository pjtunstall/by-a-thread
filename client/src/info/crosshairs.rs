use macroquad::prelude::*;

const CROSSHAIR_SIZE: f32 = 20.0;
const CROSSHAIR_THICKNESS: f32 = 2.0;
const CROSSHAIR_COLOR: Color = BLACK;

pub fn draw_crosshairs() {
    let screen_center = vec2(screen_width() / 2.0, screen_height() / 2.0);

    let half_size = CROSSHAIR_SIZE / 2.0;
    let thickness = CROSSHAIR_THICKNESS;

    // Horizontal line.
    draw_rectangle(
        screen_center.x - half_size,
        screen_center.y - thickness / 2.0,
        CROSSHAIR_SIZE,
        thickness,
        CROSSHAIR_COLOR,
    );

    // Vertical line.
    draw_rectangle(
        screen_center.x - thickness / 2.0,
        screen_center.y - half_size,
        thickness,
        CROSSHAIR_SIZE,
        CROSSHAIR_COLOR,
    );
}
