use macroquad::{
    Error,
    audio::{Sound, load_sound_from_bytes},
    prelude::*,
};

pub struct Assets {
    pub font: Font,
    pub map_font: Font,
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
        #[cfg(target_os = "windows")]
        use embedded_assets::*;

        #[cfg(not(target_os = "windows"))]
        use file_assets::*;

        Self {
            font: load_font().await,
            map_font: load_map_font().await,
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

#[cfg(target_os = "windows")]
pub mod embedded_assets {
    use super::*;

    pub async fn load_font() -> Font {
        load_ttf_font_from_bytes(include_bytes!(
            "../assets/fonts/PF Hellenica Serif Pro Bold.ttf"
        ))
        .expect("failed to load font")
    }

    pub async fn load_map_font() -> Font {
        load_ttf_font_from_bytes(include_bytes!("../assets/fonts/NotoSerifBold-MmDx.ttf"))
            .expect("failed to load map font")
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
        let font_path =
            std::path::Path::new("/usr/lib/by-a-thread/fonts/PF Hellenica Serif Pro Bold.ttf");
        if font_path.exists() {
            load_ttf_font("/usr/lib/by-a-thread/fonts/PF Hellenica Serif Pro Bold.ttf")
                .await
                .expect("failed to load font")
        } else {
            load_ttf_font("client/assets/fonts/PF Hellenica Serif Pro Bold.ttf")
                .await
                .expect("failed to load font")
        }
    }

    pub async fn load_map_font() -> Font {
        let font_path = std::path::Path::new("/usr/lib/by-a-thread/fonts/NotoSerifBold-MmDx.ttf");
        if font_path.exists() {
            load_ttf_font("/usr/lib/by-a-thread/fonts/NotoSerifBold-MmDx.ttf")
                .await
                .expect("failed to load map font")
        } else {
            load_ttf_font("client/assets/fonts/NotoSerifBold-MmDx.ttf")
                .await
                .expect("failed to load map font")
        }
    }

    pub async fn load_bull_texture() -> Texture2D {
        let img_path = std::path::Path::new("/usr/lib/by-a-thread/images/bull.png");
        if img_path.exists() {
            load_texture("/usr/lib/by-a-thread/images/bull.png")
                .await
                .expect("failed to load bull texture")
        } else {
            load_texture("client/assets/images/bull.png")
                .await
                .expect("failed to load bull texture")
        }
    }

    pub async fn load_ball_texture() -> Texture2D {
        let img_path = std::path::Path::new("/usr/lib/by-a-thread/images/ball.png");
        if img_path.exists() {
            load_texture("/usr/lib/by-a-thread/images/ball.png")
                .await
                .expect("failed to load ball texture")
        } else {
            load_texture("client/assets/images/ball.png")
                .await
                .expect("failed to load ball texture")
        }
    }

    pub async fn load_griffin_texture() -> Texture2D {
        let img_path = std::path::Path::new("/usr/lib/by-a-thread/images/griffin.png");
        if img_path.exists() {
            load_texture("/usr/lib/by-a-thread/images/griffin.png")
                .await
                .expect("failed to load griffin texture")
        } else {
            load_texture("client/assets/images/griffin.png")
                .await
                .expect("failed to load griffin texture")
        }
    }

    pub async fn load_blue_rust_texture() -> Texture2D {
        let img_path = std::path::Path::new("/usr/lib/by-a-thread/images/rust-blue.png");
        if img_path.exists() {
            load_texture("/usr/lib/by-a-thread/images/rust-blue.png")
                .await
                .expect("failed to load blue rust texture")
        } else {
            load_texture("client/assets/images/rust-blue.png")
                .await
                .expect("failed to load blue rust texture")
        }
    }

    pub async fn load_white_rust_texture() -> Texture2D {
        let img_path = std::path::Path::new("/usr/lib/by-a-thread/images/rust-white.png");
        if img_path.exists() {
            load_texture("/usr/lib/by-a-thread/images/rust-white.png")
                .await
                .expect("failed to load white rust texture")
        } else {
            load_texture("client/assets/images/rust-white.png")
                .await
                .expect("failed to load white rust texture")
        }
    }

    pub async fn load_gun_sound() -> Sound {
        let sfx_path = std::path::Path::new("/usr/lib/by-a-thread/sfx/gun.wav");
        if sfx_path.exists() {
            load_sfx_from_file("/usr/lib/by-a-thread/sfx/gun.wav")
                .await
                .expect("failed to load gun sound")
        } else {
            load_sfx_from_file("client/assets/sfx/gun.wav")
                .await
                .expect("failed to load gun sound")
        }
    }

    pub async fn load_clang() -> Sound {
        let sfx_path = std::path::Path::new("/usr/lib/by-a-thread/sfx/clang.wav");
        if sfx_path.exists() {
            load_sfx_from_file("/usr/lib/by-a-thread/sfx/clang.wav")
                .await
                .expect("failed to load clang sound")
        } else {
            load_sfx_from_file("client/assets/sfx/clang.wav")
                .await
                .expect("failed to load clang sound")
        }
    }

    pub async fn load_deep_clang() -> Sound {
        let sfx_path = std::path::Path::new("/usr/lib/by-a-thread/sfx/deep_clang.wav");
        if sfx_path.exists() {
            load_sfx_from_file("/usr/lib/by-a-thread/sfx/deep_clang.wav")
                .await
                .expect("failed to load deep clang sound")
        } else {
            load_sfx_from_file("client/assets/sfx/deep_clang.wav")
                .await
                .expect("failed to load deep clang sound")
        }
    }

    pub async fn load_shatter_sound() -> Sound {
        let sfx_path = std::path::Path::new("/usr/lib/by-a-thread/sfx/shatter.wav");
        if sfx_path.exists() {
            load_sfx_from_file("/usr/lib/by-a-thread/sfx/shatter.wav")
                .await
                .expect("failed to load shatter sound")
        } else {
            load_sfx_from_file("client/assets/sfx/shatter.wav")
                .await
                .expect("failed to load shatter sound")
        }
    }

    pub async fn load_bell_sound() -> Sound {
        let sfx_path = std::path::Path::new("/usr/lib/by-a-thread/sfx/bell.wav");
        if sfx_path.exists() {
            load_sfx_from_file("/usr/lib/by-a-thread/sfx/bell.wav")
                .await
                .expect("failed to load bell sound")
        } else {
            load_sfx_from_file("client/assets/sfx/bell.wav")
                .await
                .expect("failed to load bell sound")
        }
    }
}

#[allow(dead_code)]
async fn load_sfx_from_file(path: &str) -> Result<Sound, Error> {
    let bytes = macroquad::file::load_file(path).await?;
    Ok(load_sound_from_bytes(&bytes).await?)
}
