use std::f32::consts::PI;

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
    let slices = 32;
    let stacks = 16;
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

pub fn sky_colors(level: u8) -> [[u8; 4]; 3] {
    match level {
        2 => {
            let c1 = [34, 139, 34, 255];
            let c2 = [144, 238, 144, 255];
            let c3 = [0, 100, 0, 255];
            [c1, c2, c3]
        }
        3 => {
            let c1 = [255, 165, 0, 255];
            let c2 = [0, 255, 0, 255];
            let c3 = [255, 255, 255, 255];
            [c1, c2, c3]
        }
        4 => {
            let c1 = [255, 255, 255, 255];
            let c2 = [0, 0, 0, 0];
            let c3 = [0, 0, 0, 255];
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
