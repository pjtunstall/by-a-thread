use macroquad::{
    Error,
    audio::{Sound, load_sound_from_bytes},
    prelude::*,
};

use common::maze::CELL_SIZE;

pub struct Assets {
    pub font: Font,
    pub floor_texture: Texture2D,
    pub bull_texture: Texture2D,
    pub ball_texture: Texture2D,
    pub griffin_texture: Texture2D,
    pub dolphins_texture: Texture2D,
    pub gun_sound: Sound,
    pub clang: Sound,
    pub deep_clang: Sound,
    pub shatter_sound: Sound,
    pub bell_sound: Sound,
}

impl Assets {
    pub async fn load() -> Self {
        let font =
            load_ttf_font_from_bytes(include_bytes!("../assets/fonts/NotoSerifBold-MmDx.ttf"))
                .expect("failed to load font");

        let bull_bytes = include_bytes!("../assets/images/bull.png");
        let bull_texture = Texture2D::from_file_with_format(bull_bytes, None);

        let ball_bytes = include_bytes!("../assets/images/ball.png");
        let ball_texture = Texture2D::from_file_with_format(ball_bytes, None);

        let griffin_bytes = include_bytes!("../assets/images/griffin.png");
        let griffin_texture = Texture2D::from_file_with_format(griffin_bytes, None);

        let dolphins_bytes = include_bytes!("../assets/images/dolphins.png");
        let dolphins_texture = Texture2D::from_file_with_format(dolphins_bytes, None);

        let floor_texture = generate_floor_texture();

        let gun_sound = load_sfx(include_bytes!("../assets/sfx/gun.wav"))
            .await
            .expect("failed to load gun sound");
        let clang = load_sfx(include_bytes!("../assets/sfx/clang.wav"))
            .await
            .expect("failed to load clang sound");
        let deep_clang = load_sfx(include_bytes!("../assets/sfx/deep_clang.wav"))
            .await
            .expect("failed to load deep clang sound");
        let shatter_sound = load_sfx(include_bytes!("../assets/sfx/shatter.wav"))
            .await
            .expect("failed to load shatter sound");
        let bell_sound = load_sfx(include_bytes!("../assets/sfx/bell.wav"))
            .await
            .expect("failed to load bell sound");

        Self {
            font,
            floor_texture,
            bull_texture,
            ball_texture,
            griffin_texture,
            dolphins_texture,
            gun_sound,
            clang,
            deep_clang,
            shatter_sound,
            bell_sound,
        }
    }
}

pub fn generate_floor_texture() -> Texture2D {
    let half_check_size = 8.0;
    let check_size = 2.0 * half_check_size;
    let checks_per_cell = (CELL_SIZE / check_size).round() as u16;

    let mut image = Image::gen_image_color(checks_per_cell, checks_per_cell, BEIGE);

    for y in 0..checks_per_cell {
        for x in 0..checks_per_cell {
            if (x + y) % 2 != 0 {
                image.set_pixel(x as u32, y as u32, BROWN);
            }
        }
    }

    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    texture
}

async fn load_sfx(bytes: &[u8]) -> Result<Sound, Error> {
    Ok(load_sound_from_bytes(bytes).await?)
}
