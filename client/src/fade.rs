use macroquad::prelude::*;

pub fn new_fade_to_black() -> Fade {
    Fade::new(get_fade_to_black_color, 5.5, 0.25)
}

pub fn new_flash() -> Fade {
    Fade::new(get_flash_color, 1.0, 0.5)
}

fn get_fade_to_black_color(fade: f32) -> Color {
    Color::new(fade, fade, fade, 1.0 - fade)
}

fn get_flash_color(fade: f32) -> Color {
    Color::new(1.0, fade, fade, fade)
}

pub struct Fade {
    pub start_time: f64,
    get_color: fn(f32) -> Color,
    duration: f64,
    power: f32,
}

impl Fade {
    pub fn new(get_color: fn(f32) -> Color, duration: f64, power: f32) -> Self {
        let start_time = miniquad::date::now();
        Fade {
            start_time,
            get_color,
            duration,
            power,
        }
    }

    pub fn is_still_fading_so_draw(&self) -> bool {
        let now = miniquad::date::now();
        let elapsed = now - self.start_time;
        if elapsed < self.duration {
            let fade = 1.0 - ((elapsed as f32) / (self.duration as f32)).powf(self.power);
            push_camera_state();
            set_default_camera();

            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                (self.get_color)(fade),
            );

            pop_camera_state();
            true
        } else {
            false
        }
    }
}
