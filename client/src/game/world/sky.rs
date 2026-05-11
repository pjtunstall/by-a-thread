use std::f32::consts::PI;

use ::rand::{Rng, SeedableRng, rngs::StdRng};
use macroquad::prelude::*;

pub struct Sky {
    pub mesh: Mesh,
}

impl Sky {
    pub fn new(texture: Option<Texture2D>, sky_colors: [[u8; 4]; 3]) -> Self {
        let mesh = generate_sky(texture, sky_colors);

        Sky { mesh }
    }

    pub fn draw(&self) {
        draw_mesh(&self.mesh);
    }
}

pub fn generate_sky(texture: Option<Texture2D>, sky_colors: [[u8; 4]; 3]) -> Mesh {
    let radius = 4096.0;
    let slices = SLICES as usize;
    let stacks = STACKS as usize;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let [c1, c2, c3] = sky_colors;

    for i in 0..=stacks {
        let phi = (i as f32 / stacks as f32) * (PI / 2.0); // Vertical.
        for j in 0..=slices {
            let theta = j as f32 / slices as f32 * 2.0 * PI; // Horizontal.

            let x = radius * theta.sin() * phi.sin();
            let y = radius * phi.cos();
            let z = radius * theta.cos() * phi.sin();

            let normal = vec3(x, y, z).normalize();

            let u = j as f32 / slices as f32;
            let v = i as f32 / stacks as f32;

            let horizontal_t = if theta <= PI {
                theta / PI
            } else {
                2.0 - theta / PI
            };
            let vertical_t = phi.cos();

            let horizontal_color = lerp_color(c1, c2, horizontal_t);

            let color = lerp_color(horizontal_color, c3, vertical_t);

            vertices.push(Vertex {
                position: vec3(x, y, z),
                uv: vec2(u, v),
                color,
                normal: vec4(normal.x, normal.y, normal.z, 0.0),
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

const SLICES: u32 = 32;
const STACKS: u32 = 16;
const TILE_SIZE: u32 = 64;
const STARS_PER_TILE: usize = 3;

pub fn generate_starfield_texture() -> Texture2D {
    generate_starfield_atlas_texture(SLICES, STACKS)
}

fn generate_starfield_atlas_texture(slices: u32, stacks: u32) -> Texture2D {
    let width = slices * TILE_SIZE;
    let height = stacks * TILE_SIZE;
    let black = Color::from_rgba(0, 0, 0, 255);
    let deep_blue = Color::from_rgba(15, 25, 90, 255);

    let mut image = Image::gen_image_color(width as u16, height as u16, black);
    for ty in 0..height {
        let t = ty as f32 / (height - 1).max(1) as f32;
        let r = (black.r as f32 + (deep_blue.r as f32 - black.r as f32) * t) as u8;
        let g = (black.g as f32 + (deep_blue.g as f32 - black.g as f32) * t) as u8;
        let b = (black.b as f32 + (deep_blue.b as f32 - black.b as f32) * t) as u8;
        let row_color = Color::from_rgba(r, g, b, 255);
        for tx in 0..width {
            image.set_pixel(tx, ty, row_color);
        }
    }

    for stack in 0..stacks {
        for slice in 0..slices {
            let seed = 9u64
                .wrapping_add(stack as u64 * 1000)
                .wrapping_add(slice as u64);
            let mut rng = StdRng::seed_from_u64(seed);
            let base_x = slice * TILE_SIZE;
            let base_y = stack * TILE_SIZE;
            for _ in 0..STARS_PER_TILE {
                let x = base_x + rng.random_range(0..TILE_SIZE);
                let y = base_y + rng.random_range(0..TILE_SIZE);
                let brightness = match rng.random_range(0..100) {
                    0..=4 => 255,
                    5..=14 => 220,
                    15..=29 => 180,
                    30..=49 => 140,
                    _ => 100,
                };
                let star_color = Color::from_rgba(brightness, brightness, brightness, 255);
                image.set_pixel(x, y, star_color);
            }
        }
    }

    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

pub fn sky_colors(level: u8) -> [[u8; 4]; 3] {
    match level {
        0 => {
            let c1 = [255, 240, 220, 255];
            let c2 = [200, 210, 255, 255];
            let c3 = [120, 150, 255, 255];
            [c1, c2, c3]
        }
        1 => {
            let c1 = [255, 255, 0, 255];
            let c2 = [0, 0, 0, 0];
            let c3 = [0, 0, 255, 255];
            [c1, c2, c3]
        }
        2 => {
            let c1 = [200, 180, 255, 255];
            let c2 = [230, 220, 255, 255];
            let c3 = [140, 160, 220, 255];
            [c1, c2, c3]
        }
        3 => {
            let c1 = [34, 139, 34, 255];
            let c2 = [144, 238, 144, 255];
            let c3 = [0, 100, 0, 255];
            [c1, c2, c3]
        }
        4 => {
            let c1 = [255, 165, 0, 255];
            let c2 = [0, 255, 0, 255];
            let c3 = [255, 255, 255, 255];
            [c1, c2, c3]
        }
        5 => {
            let c1 = [255, 255, 255, 255];
            let c2 = [0, 0, 0, 0];
            let c3 = [0, 0, 0, 255];
            [c1, c2, c3]
        }
        6 => {
            let c1 = [180, 140, 100, 255];
            let c2 = [220, 200, 170, 255];
            let c3 = [120, 90, 60, 255];
            [c1, c2, c3]
        }
        7 => {
            let c1 = [255, 255, 0, 255];
            let c2 = [0, 0, 0, 0];
            let c3 = [0, 0, 255, 255];
            [c1, c2, c3]
        }
        8 => {
            let c1 = [112, 128, 144, 255];
            let c2 = [176, 196, 222, 255];
            let c3 = [47, 79, 79, 255];
            [c1, c2, c3]
        }
        9 => {
            let c1 = [255, 255, 255, 255];
            let c2 = [255, 255, 255, 255];
            let c3 = [255, 255, 255, 255];
            [c1, c2, c3]
        }
        _ => {
            let c1 = [255, 255, 0, 255];
            let c2 = [0, 0, 0, 0];
            let c3 = [0, 0, 255, 255];
            [c1, c2, c3]
        }
    }
}

fn lerp_color(c1: [u8; 4], c2: [u8; 4], t: f32) -> [u8; 4] {
    [
        (c1[0] as f32 + (c2[0] as f32 - c1[0] as f32) * t) as u8,
        (c1[1] as f32 + (c2[1] as f32 - c1[1] as f32) * t) as u8,
        (c1[2] as f32 + (c2[2] as f32 - c1[2] as f32) * t) as u8,
        (c1[3] as f32 + (c2[3] as f32 - c1[3] as f32) * t) as u8,
    ]
}
