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

#[cfg(target_os = "windows")]
mod embedded_assets {
    use super::*;

    pub async fn load_font() -> Font {
        load_ttf_font_from_bytes(include_bytes!("../assets/fonts/NotoSerifBold-MmDx.ttf"))
            .expect("failed to load font")
    }

    pub async fn load_bull_texture() -> Texture2D {
        let bull_bytes = include_bytes!("../assets/images/bull.png");
        Texture2D::from_file_with_format(bull_bytes, None)
    }

    pub async fn load_ball_texture() -> Texture2D {
        let ball_bytes = include_bytes!("../assets/images/ball.png");
        Texture2D::from_file_with_format(ball_bytes, None)
    }

    pub async fn load_griffin_texture() -> Texture2D {
        let griffin_bytes = include_bytes!("../assets/images/griffin.png");
        Texture2D::from_file_with_format(griffin_bytes, None)
    }

    pub async fn load_blue_rust_texture() -> Texture2D {
        let blue_rust_bytes = include_bytes!("../assets/images/rust-blue.png");
        Texture2D::from_file_with_format(blue_rust_bytes, None)
    }

    pub async fn load_white_rust_texture() -> Texture2D {
        let white_rust_bytes = include_bytes!("../assets/images/rust-white.png");
        Texture2D::from_file_with_format(white_rust_bytes, None)
    }

    pub async fn load_gun_sound() -> Sound {
        let bytes = include_bytes!("../assets/sfx/gun.wav");
        load_sound_from_bytes(bytes)
            .await
            .expect("failed to load gun sound")
    }

    pub async fn load_clang() -> Sound {
        let bytes = include_bytes!("../assets/sfx/clang.wav");
        load_sound_from_bytes(bytes)
            .await
            .expect("failed to load clang sound")
    }

    pub async fn load_deep_clang() -> Sound {
        let bytes = include_bytes!("../assets/sfx/deep_clang.wav");
        load_sound_from_bytes(bytes)
            .await
            .expect("failed to load deep clang sound")
    }

    pub async fn load_shatter_sound() -> Sound {
        let bytes = include_bytes!("../assets/sfx/shatter.wav");
        load_sound_from_bytes(bytes)
            .await
            .expect("failed to load shatter sound")
    }

    pub async fn load_bell_sound() -> Sound {
        let bytes = include_bytes!("../assets/sfx/bell.wav");
        load_sound_from_bytes(bytes)
            .await
            .expect("failed to load bell sound")
    }
}

#[cfg(not(target_os = "windows"))]
mod file_assets {
    use super::*;

    pub async fn load_font() -> Font {
        load_ttf_font("assets/fonts/NotoSerifBold-MmDx.ttf")
            .await
            .expect("failed to load font")
    }

    pub async fn load_bull_texture() -> Texture2D {
        load_texture("assets/images/bull.png")
            .await
            .expect("failed to load bull texture")
    }

    pub async fn load_ball_texture() -> Texture2D {
        load_texture("assets/images/ball.png")
            .await
            .expect("failed to load ball texture")
    }

    pub async fn load_griffin_texture() -> Texture2D {
        load_texture("assets/images/griffin.png")
            .await
            .expect("failed to load griffin texture")
    }

    pub async fn load_blue_rust_texture() -> Texture2D {
        load_texture("assets/images/rust-blue.png")
            .await
            .expect("failed to load blue rust texture")
    }

    pub async fn load_white_rust_texture() -> Texture2D {
        load_texture("assets/images/rust-white.png")
            .await
            .expect("failed to load white rust texture")
    }

    pub async fn load_gun_sound() -> Sound {
        load_sfx_from_file("assets/sfx/gun.wav")
            .await
            .expect("failed to load gun sound")
    }

    pub async fn load_clang() -> Sound {
        load_sfx_from_file("assets/sfx/clang.wav")
            .await
            .expect("failed to load clang sound")
    }

    pub async fn load_deep_clang() -> Sound {
        load_sfx_from_file("assets/sfx/deep_clang.wav")
            .await
            .expect("failed to load deep clang sound")
    }

    pub async fn load_shatter_sound() -> Sound {
        load_sfx_from_file("assets/sfx/shatter.wav")
            .await
            .expect("failed to load shatter sound")
    }

    pub async fn load_bell_sound() -> Sound {
        load_sfx_from_file("assets/sfx/bell.wav")
            .await
            .expect("failed to load bell sound")
    }
}

impl Assets {
    pub async fn load() -> Self {
        #[cfg(target_os = "windows")]
        use embedded_assets::*;

        #[cfg(not(target_os = "windows"))]
        use file_assets::*;

        Self {
            font: load_font().await,
            bull_texture: load_bull_texture().await,
            ball_texture: load_ball_texture().await,
            griffin_texture: load_griffin_texture().await,
            blue_rust_texture: load_blue_rust_texture().await,
            white_rust_texture: load_white_rust_texture().await,
            gun_sound: load_gun_sound().await,
            clang: load_clang().await,
            deep_clang: load_deep_clang().await,
            shatter_sound: load_shatter_sound().await,
            bell_sound: load_bell_sound().await,
        }
    }
}

async fn load_sfx_from_file(path: &str) -> Result<Sound, Error> {
    let bytes = macroquad::file::load_file(path).await?;
    Ok(load_sound_from_bytes(&bytes).await?)
}
