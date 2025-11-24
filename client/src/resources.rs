use macroquad::prelude::*;

pub struct Resources {
    pub wall_texture: Texture2D,
}

impl Resources {
    pub async fn load() -> Self {
        let bytes = include_bytes!("../assets/bull.png");

        let wall_texture = Texture2D::from_file_with_format(bytes, None);

        Self { wall_texture }
    }
}
