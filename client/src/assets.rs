use macroquad::{
    Error,
    audio::{Sound, load_sound_from_bytes},
    prelude::*,
};

pub struct Assets {
    pub font: Font,
    pub bull_texture: Texture2D,
    pub ball_texture: Texture2D,
    pub griffin_texture: Texture2D,
    pub blue_rust_texture: Texture2D,
    pub white_rust_texture: Texture2D,
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

        let blue_rust_bytes = include_bytes!("../assets/images/rust-blue.png");
        let blue_rust_texture = Texture2D::from_file_with_format(blue_rust_bytes, None);

        let white_rust_bytes = include_bytes!("../assets/images/rust-white.png");
        let white_rust_texture = Texture2D::from_file_with_format(white_rust_bytes, None);

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
            bull_texture,
            ball_texture,
            griffin_texture,
            blue_rust_texture,
            white_rust_texture,
            gun_sound,
            clang,
            deep_clang,
            shatter_sound,
            bell_sound,
        }
    }
}

async fn load_sfx(bytes: &[u8]) -> Result<Sound, Error> {
    Ok(load_sound_from_bytes(bytes).await?)
}
