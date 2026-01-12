use macroquad::prelude::*;

use common::maze::CELL_SIZE;

pub struct Assets {
    pub font: Font,
    pub floor_texture: Texture2D,
    pub wall_texture: Texture2D,
    pub ball_texture: Texture2D,
}

impl Assets {
    pub async fn load() -> Self {
        let font =
            load_ttf_font_from_bytes(include_bytes!("../assets/fonts/NotoSerifBold-MmDx.ttf"))
                .expect("failed to load font");

        let wall_bytes = include_bytes!("../assets/bull.png");
        let wall_texture = Texture2D::from_file_with_format(wall_bytes, None);

        let ball_bytes = include_bytes!("../assets/ball.png");
        let ball_texture = Texture2D::from_file_with_format(ball_bytes, None);

        let floor_texture = generate_floor_texture();

        Self {
            font,
            floor_texture,
            wall_texture,
            ball_texture,
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
