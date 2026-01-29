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
    use std::path::PathBuf;

    fn bundle_resources_base() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            let exe = std::env::current_exe().ok()?;
            if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
                let path = exe.parent()?.parent()?.parent()?.join("Resources");
                (path.exists() && path.is_dir()).then_some(path)
            } else {
                None
            }
        }
        #[cfg(not(target_os = "macos"))]
        None
    }

    fn resource_path(subdir: &str, name: &str) -> PathBuf {
        if let Some(base) = bundle_resources_base() {
            let p = base.join(subdir).join(name);
            if p.exists() {
                return p;
            }
        }
        let usr = PathBuf::from("/usr/lib/by-a-thread")
            .join(subdir)
            .join(name);
        if usr.exists() {
            return usr;
        }
        PathBuf::from("client/assets").join(subdir).join(name)
    }

    pub async fn load_font() -> Font {
        let path = resource_path("fonts", "PF Hellenica Serif Pro Bold.ttf");
        load_ttf_font(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load font")
    }

    pub async fn load_map_font() -> Font {
        let path = resource_path("fonts", "NotoSerifBold-MmDx.ttf");
        load_ttf_font(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load map font")
    }

    pub async fn load_bull_texture() -> Texture2D {
        let path = resource_path("images", "bull.png");
        load_texture(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load bull texture")
    }

    pub async fn load_ball_texture() -> Texture2D {
        let path = resource_path("images", "ball.png");
        load_texture(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load ball texture")
    }

    pub async fn load_griffin_texture() -> Texture2D {
        let path = resource_path("images", "griffin.png");
        load_texture(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load griffin texture")
    }

    pub async fn load_blue_rust_texture() -> Texture2D {
        let path = resource_path("images", "rust-blue.png");
        load_texture(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load blue rust texture")
    }

    pub async fn load_white_rust_texture() -> Texture2D {
        let path = resource_path("images", "rust-white.png");
        load_texture(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load white rust texture")
    }

    pub async fn load_gun_sound() -> Sound {
        let path = resource_path("sfx", "gun.wav");
        load_sfx_from_file(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load gun sound")
    }

    pub async fn load_clang() -> Sound {
        let path = resource_path("sfx", "clang.wav");
        load_sfx_from_file(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load clang sound")
    }

    pub async fn load_deep_clang() -> Sound {
        let path = resource_path("sfx", "deep_clang.wav");
        load_sfx_from_file(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load deep clang sound")
    }

    pub async fn load_shatter_sound() -> Sound {
        let path = resource_path("sfx", "shatter.wav");
        load_sfx_from_file(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load shatter sound")
    }

    pub async fn load_bell_sound() -> Sound {
        let path = resource_path("sfx", "bell.wav");
        load_sfx_from_file(path.to_string_lossy().as_ref())
            .await
            .expect("failed to load bell sound")
    }
}

#[allow(dead_code)]
async fn load_sfx_from_file(path: &str) -> Result<Sound, Error> {
    let bytes = macroquad::file::load_file(path).await?;
    Ok(load_sound_from_bytes(&bytes).await?)
}
