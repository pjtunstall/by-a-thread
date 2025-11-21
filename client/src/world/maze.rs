use macroquad::prelude::*;

pub use shared::maze::{CELL_SIZE, Maze};

pub trait MazeExtension {
    fn draw(&self, wall_texture: &Texture2D);
}

impl MazeExtension for Maze {
    fn draw(&self, wall_texture: &Texture2D) {
        let grid_len = self.grid.len();

        for x in 0..grid_len {
            for z in 0..grid_len {
                let corner_x = (x as f32) * CELL_SIZE;
                let corner_z = (z as f32) * CELL_SIZE;

                if self.grid[z][x] == 0 {
                    continue;
                }

                // Necessary because Macrquad's built-in `draw_cube` function doesn't orient faces in a way that will keep the texture the right way up for all of them.
                draw_custom_cuboid(
                    vec3(corner_x + CELL_SIZE / 2.0, 32.0, corner_z + CELL_SIZE / 2.0),
                    vec3(CELL_SIZE, CELL_SIZE, CELL_SIZE),
                    wall_texture,
                    WHITE,
                );
            }
        }
    }
}

fn draw_custom_cuboid(position: Vec3, size: Vec3, texture: &Texture2D, color: Color) {
    let half_width = size.x / 2.0;
    let half_height = size.y / 2.0;
    let half_depth = size.z / 2.0;

    let x = position.x;
    let y = position.y;
    let z = position.z;

    let vertices = [
        // Front face.
        vec3(x - half_width, y - half_height, z + half_depth), // Bottom-left-front (0).
        vec3(x + half_width, y - half_height, z + half_depth), // Bottom-right-front (1).
        vec3(x + half_width, y + half_height, z + half_depth), // Top-right-front (2).
        vec3(x - half_width, y + half_height, z + half_depth), // Top-left-front (3).
        // Back face
        vec3(x - half_width, y - half_height, z - half_depth), // Bottom-left-back (4).
        vec3(x + half_width, y - half_height, z - half_depth), // Bottom-right-back (5).
        vec3(x + half_width, y + half_height, z - half_depth), // Top-right-back (6).
        vec3(x - half_width, y + half_height, z - half_depth), // Top-left-back (7).
    ];

    // Texture coordinates.
    let tex_coords = [
        vec2(0.0, 1.0), // Bottom-left.
        vec2(1.0, 1.0), // Bottom-right.
        vec2(1.0, 0.0), // Top-right.
        vec2(0.0, 0.0), // Top-left.
    ];

    let mut vertices_data: Vec<macroquad::models::Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    let mut add_face =
        |v1_idx: usize, v2_idx: usize, v3_idx: usize, v4_idx: usize, normal: Vec3| {
            let base_idx = vertices_data.len() as u16;

            // Convert normal from `Vec3` to `Vec4` (w=0).
            let normal_vec4 = vec4(normal.x, normal.y, normal.z, 0.0);

            // Convert color to `[u8; 4]`.
            let color_array: [u8; 4] = color.into();

            // Add vertices for this face.
            vertices_data.push(macroquad::models::Vertex {
                position: vertices[v1_idx],
                normal: normal_vec4,
                uv: tex_coords[0],
                color: color_array,
            });
            vertices_data.push(macroquad::models::Vertex {
                position: vertices[v2_idx],
                normal: normal_vec4,
                uv: tex_coords[1],
                color: color_array,
            });
            vertices_data.push(macroquad::models::Vertex {
                position: vertices[v3_idx],
                normal: normal_vec4,
                uv: tex_coords[2],
                color: color_array,
            });
            vertices_data.push(macroquad::models::Vertex {
                position: vertices[v4_idx],
                normal: normal_vec4,
                uv: tex_coords[3],
                color: color_array,
            });

            // Add indices for the two triangles that make up the face.
            indices.push(base_idx);
            indices.push(base_idx + 1);
            indices.push(base_idx + 2);

            indices.push(base_idx);
            indices.push(base_idx + 2);
            indices.push(base_idx + 3);
        };

    // Front face (0,1,2,3).
    add_face(0, 1, 2, 3, vec3(0.0, 0.0, 1.0));

    // Back face (5,4,7,6).
    add_face(5, 4, 7, 6, vec3(0.0, 0.0, -1.0));

    // Right face (1,5,6,2).
    add_face(1, 5, 6, 2, vec3(1.0, 0.0, 0.0));

    // Left face (4,0,3,7).
    add_face(4, 0, 3, 7, vec3(-1.0, 0.0, 0.0));

    // Top face (3,2,6,7).
    add_face(3, 2, 6, 7, vec3(0.0, 1.0, 0.0));

    // Bottom face (4,5,1,0).
    add_face(4, 5, 1, 0, vec3(0.0, -1.0, 0.0));

    let mesh = Mesh {
        vertices: vertices_data,
        indices,
        texture: Some(texture.clone()),
    };

    draw_mesh(&mesh);
}
