use std::f32::consts::PI;

use macroquad::prelude::*;

use common::{maze::CELL_SIZE, player::RADIUS};

pub struct Assets {
    pub floor_texture: Texture2D,
    pub wall_texture: Texture2D,
    pub ball_mesh: Mesh,
}

impl Assets {
    pub async fn load() -> Self {
        let wall_bytes = include_bytes!("../assets/bull.png");
        let wall_texture = Texture2D::from_file_with_format(wall_bytes, None);

        let ball_bytes = include_bytes!("../assets/ball.png");
        let ball_texture = Texture2D::from_file_with_format(ball_bytes, None);
        let ball_mesh = generate_ball_mesh(Some(ball_texture));

        let floor_texture = generate_floor_texture();

        Self {
            floor_texture,
            wall_texture,
            ball_mesh,
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

pub fn generate_ball_mesh(texture: Option<Texture2D>) -> Mesh {
    let slices = 32;
    let stacks = 16;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let phi = (i as f32 / stacks as f32) * PI; // Vertical.
        for j in 0..=slices {
            let theta = j as f32 / slices as f32 * 2.0 * PI; // Horizontal.

            // Coordinates relative to the center of the sphere.
            let x = RADIUS * theta.sin() * phi.sin();
            let y = RADIUS * phi.cos();
            let z = RADIUS * theta.cos() * phi.sin();

            let u = j as f32 / slices as f32;
            let v = i as f32 / stacks as f32;

            vertices.push(Vertex {
                position: vec3(x, y, z),
                uv: vec2(u, v),
                color: [255, 255, 255, 255],
                normal: vec4(x, y, z, 0.0),
            });

            if i < stacks && j < slices {
                let a = i as u16 * (slices as u16 + 1) + j as u16;
                let b = a + slices as u16 + 1;

                indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            }
        }
    }

    Mesh {
        vertices,
        indices,
        texture,
    }
}
