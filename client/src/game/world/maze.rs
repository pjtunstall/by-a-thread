use std::fmt;

use macroquad::prelude::*;

pub use common::maze::{CELL_SIZE, Maze};

pub struct MazeMeshes {
    pub walls: Vec<Mesh>,
    pub floor: Vec<Mesh>,
    pub shadows: Vec<Mesh>,
}

impl fmt::Debug for MazeMeshes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MazeMeshes")
            .field("walls_count", &self.walls.len())
            .field("floor_count", &self.floor.len())
            .field("shadows_count", &self.shadows.len())
            .finish()
    }
}

pub trait MazeExtension {
    fn draw(&self, meshes: &MazeMeshes);
}

impl MazeExtension for Maze {
    fn draw(&self, meshes: &MazeMeshes) {
        // Draw Floor
        for mesh in &meshes.floor {
            draw_mesh(mesh);
        }
        // Draw Shadows
        for mesh in &meshes.shadows {
            draw_mesh(mesh);
        }
        // Draw Walls
        for mesh in &meshes.walls {
            draw_mesh(mesh);
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

// TODO: Investigate why the shadow is flickery with movement.
pub fn build_maze_meshes(
    maze: &Maze,
    wall_texture: &Texture2D,
    floor_texture: &Texture2D,
) -> MazeMeshes {
    let height = maze.grid.len();
    let width = if height > 0 { maze.grid[0].len() } else { 0 };

    const MAX_VERTICES: usize = 2_000;

    let mut wall_builder = MeshBuilder::new(wall_texture.clone(), MAX_VERTICES);
    let mut floor_builder = MeshBuilder::new(floor_texture.clone(), MAX_VERTICES);
    let mut shadow_builder = MeshBuilder::new(Texture2D::empty(), MAX_VERTICES);

    let w_size = vec3(CELL_SIZE, CELL_SIZE, CELL_SIZE);
    let w_hw = w_size.x / 2.0;
    let w_hh = w_size.y / 2.0;
    let w_hd = w_size.z / 2.0;

    let wall_verts = [
        vec3(-w_hw, -w_hh, w_hd),
        vec3(w_hw, -w_hh, w_hd),
        vec3(w_hw, w_hh, w_hd),
        vec3(-w_hw, w_hh, w_hd),
        vec3(-w_hw, -w_hh, -w_hd),
        vec3(w_hw, -w_hh, -w_hd),
        vec3(w_hw, w_hh, -w_hd),
        vec3(-w_hw, w_hh, -w_hd),
    ];
    let wall_uvs = [
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
    ];

    let f_hw = CELL_SIZE / 2.0;
    let f_hd = CELL_SIZE / 2.0;
    let floor_verts_local = [
        vec3(-f_hw, 0.0, f_hd),
        vec3(f_hw, 0.0, f_hd),
        vec3(f_hw, 0.0, -f_hd),
        vec3(-f_hw, 0.0, -f_hd),
    ];
    let floor_uvs = [
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
    ];

    const SHADOW_HEIGHT: f32 = 0.12;
    let shadow_color = Color::new(0.0, 0.0, 0.0, 0.3);
    let shadow_inner = CELL_SIZE / 2.0;
    let shadow_outer = shadow_inner + 2.0;
    let shadow_overhang = shadow_outer - shadow_inner;

    let add_shadow_rect = |builder: &mut MeshBuilder,
                           offset: Vec3,
                           x_min: f32,
                           x_max: f32,
                           z_min: f32,
                           z_max: f32| {
        let shadow_verts_local = [
            vec3(x_min, SHADOW_HEIGHT, z_max),
            vec3(x_max, SHADOW_HEIGHT, z_max),
            vec3(x_max, SHADOW_HEIGHT, z_min),
            vec3(x_min, SHADOW_HEIGHT, z_min),
        ];
        builder.add_quad(&shadow_verts_local, &floor_uvs, offset, shadow_color);
    };

    for z in 0..height {
        for x in 0..width {
            let cell_type = maze.grid[z][x];
            let cx = (x as f32 * CELL_SIZE) + CELL_SIZE / 2.0;
            let cz = (z as f32 * CELL_SIZE) + CELL_SIZE / 2.0;

            if cell_type == 0 {
                let offset = vec3(cx, 0.0, cz);
                floor_builder.add_quad(&floor_verts_local, &floor_uvs, offset, WHITE);
            } else {
                let cy = CELL_SIZE / 2.0;
                let offset = vec3(cx, cy, cz);

                let faces = [
                    (0, 1, 2, 3, vec3(0., 0., 1.)),
                    (5, 4, 7, 6, vec3(0., 0., -1.)),
                    (1, 5, 6, 2, vec3(1., 0., 0.)),
                    (4, 0, 3, 7, vec3(-1., 0., 0.)),
                    (3, 2, 6, 7, vec3(0., 1., 0.)),
                    (4, 5, 1, 0, vec3(0., -1., 0.)),
                ];

                for (v1, v2, v3, v4, norm) in faces.iter() {
                    wall_builder.add_face_from_indices(
                        &wall_verts,
                        *v1,
                        *v2,
                        *v3,
                        *v4,
                        *norm,
                        &wall_uvs,
                        offset,
                        WHITE,
                    );
                }

                let has_wall_left = x > 0 && maze.grid[z][x - 1] != 0;
                let has_wall_right = x + 1 < width && maze.grid[z][x + 1] != 0;
                let has_wall_up = z > 0 && maze.grid[z - 1][x] != 0;
                let has_wall_down = z + 1 < height && maze.grid[z + 1][x] != 0;
                let has_wall_up_left = z > 0 && x > 0 && maze.grid[z - 1][x - 1] != 0;
                let has_wall_up_right = z > 0 && x + 1 < width && maze.grid[z - 1][x + 1] != 0;
                let has_wall_down_left = z + 1 < height && x > 0 && maze.grid[z + 1][x - 1] != 0;
                let has_wall_down_right =
                    z + 1 < height && x + 1 < width && maze.grid[z + 1][x + 1] != 0;

                let shadow_offset = vec3(cx, 0.0, cz);
                add_shadow_rect(
                    &mut shadow_builder,
                    shadow_offset,
                    -shadow_inner,
                    shadow_inner,
                    -shadow_inner,
                    shadow_inner,
                );

                if !has_wall_left {
                    let mut z_min = -shadow_inner;
                    let mut z_max = shadow_inner;
                    if has_wall_up_left {
                        z_min += shadow_overhang;
                    }
                    if has_wall_down_left {
                        z_max -= shadow_overhang;
                    }
                    if z_min < z_max {
                        add_shadow_rect(
                            &mut shadow_builder,
                            shadow_offset,
                            -shadow_outer,
                            -shadow_inner,
                            z_min,
                            z_max,
                        );
                    }
                }
                if !has_wall_right {
                    let mut z_min = -shadow_inner;
                    let mut z_max = shadow_inner;
                    if has_wall_up_right {
                        z_min += shadow_overhang;
                    }
                    if has_wall_down_right {
                        z_max -= shadow_overhang;
                    }
                    if z_min < z_max {
                        add_shadow_rect(
                            &mut shadow_builder,
                            shadow_offset,
                            shadow_inner,
                            shadow_outer,
                            z_min,
                            z_max,
                        );
                    }
                }
                if !has_wall_up {
                    let mut x_min = -shadow_inner;
                    let mut x_max = shadow_inner;
                    if has_wall_up_left {
                        x_min += shadow_overhang;
                    }
                    if has_wall_up_right {
                        x_max -= shadow_overhang;
                    }
                    if x_min < x_max {
                        add_shadow_rect(
                            &mut shadow_builder,
                            shadow_offset,
                            x_min,
                            x_max,
                            -shadow_outer,
                            -shadow_inner,
                        );
                    }
                }
                if !has_wall_down {
                    let mut x_min = -shadow_inner;
                    let mut x_max = shadow_inner;
                    if has_wall_down_left {
                        x_min += shadow_overhang;
                    }
                    if has_wall_down_right {
                        x_max -= shadow_overhang;
                    }
                    if x_min < x_max {
                        add_shadow_rect(
                            &mut shadow_builder,
                            shadow_offset,
                            x_min,
                            x_max,
                            shadow_inner,
                            shadow_outer,
                        );
                    }
                }

                if !has_wall_left && !has_wall_up && !has_wall_up_left {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        -shadow_outer,
                        -shadow_inner,
                        -shadow_outer,
                        -shadow_inner,
                    );
                }
                if has_wall_left && has_wall_up && !has_wall_up_left {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        -shadow_outer,
                        -shadow_inner,
                        -shadow_outer,
                        -shadow_inner,
                    );
                }
                if !has_wall_right && !has_wall_up && !has_wall_up_right {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        shadow_inner,
                        shadow_outer,
                        -shadow_outer,
                        -shadow_inner,
                    );
                }
                if has_wall_right && has_wall_up && !has_wall_up_right {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        shadow_inner,
                        shadow_outer,
                        -shadow_outer,
                        -shadow_inner,
                    );
                }
                if !has_wall_left && !has_wall_down && !has_wall_down_left {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        -shadow_outer,
                        -shadow_inner,
                        shadow_inner,
                        shadow_outer,
                    );
                }
                if has_wall_left && has_wall_down && !has_wall_down_left {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        -shadow_outer,
                        -shadow_inner,
                        shadow_inner,
                        shadow_outer,
                    );
                }
                if !has_wall_right && !has_wall_down && !has_wall_down_right {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        shadow_inner,
                        shadow_outer,
                        shadow_inner,
                        shadow_outer,
                    );
                }
                if has_wall_right && has_wall_down && !has_wall_down_right {
                    add_shadow_rect(
                        &mut shadow_builder,
                        shadow_offset,
                        shadow_inner,
                        shadow_outer,
                        shadow_inner,
                        shadow_outer,
                    );
                }
            }
        }
    }

    MazeMeshes {
        walls: wall_builder.finalize(),
        floor: floor_builder.finalize(),
        shadows: shadow_builder.finalize(),
    }
}

struct MeshBuilder {
    meshes: Vec<Mesh>,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    index_offset: u16,
    texture: Texture2D,
    max_verts: usize,
}

impl MeshBuilder {
    fn new(texture: Texture2D, max_verts: usize) -> Self {
        Self {
            meshes: Vec::new(),
            vertices: Vec::with_capacity(max_verts),
            indices: Vec::with_capacity(max_verts * 3 / 2),
            index_offset: 0,
            texture,
            max_verts,
        }
    }

    fn check_flush(&mut self, verts_needed: usize) {
        if self.vertices.len() + verts_needed > self.max_verts {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        self.meshes.push(Mesh {
            vertices: std::mem::take(&mut self.vertices),
            indices: std::mem::take(&mut self.indices),
            texture: Some(self.texture.clone()),
        });

        self.index_offset = 0;
        self.vertices.reserve(self.max_verts);
        self.indices.reserve(self.max_verts * 3 / 2);
    }

    fn finalize(mut self) -> Vec<Mesh> {
        self.flush();
        self.meshes
    }

    fn add_quad(&mut self, local_verts: &[Vec3; 4], uvs: &[Vec2; 4], offset: Vec3, color: Color) {
        self.check_flush(4);
        let normal = vec4(0.0, 1.0, 0.0, 0.0);
        let color_bytes: [u8; 4] = color.into();

        for i in 0..4 {
            self.vertices.push(Vertex {
                position: local_verts[i] + offset,
                normal,
                uv: uvs[i],
                color: color_bytes,
            });
        }
        self.indices.extend_from_slice(&[
            self.index_offset,
            self.index_offset + 1,
            self.index_offset + 2,
            self.index_offset,
            self.index_offset + 2,
            self.index_offset + 3,
        ]);
        self.index_offset += 4;
    }

    fn add_face_from_indices(
        &mut self,
        all_verts: &[Vec3],
        v1: usize,
        v2: usize,
        v3: usize,
        v4: usize,
        norm: Vec3,
        uvs: &[Vec2; 4],
        offset: Vec3,
        color: Color,
    ) {
        self.check_flush(4);
        let normal_vec4 = vec4(norm.x, norm.y, norm.z, 0.0);
        let color_bytes: [u8; 4] = color.into();
        let indices_lookup = [v1, v2, v3, v4];

        for i in 0..4 {
            self.vertices.push(Vertex {
                position: all_verts[indices_lookup[i]] + offset,
                normal: normal_vec4,
                uv: uvs[i],
                color: color_bytes,
            });
        }
        self.indices.extend_from_slice(&[
            self.index_offset,
            self.index_offset + 1,
            self.index_offset + 2,
            self.index_offset,
            self.index_offset + 2,
            self.index_offset + 3,
        ]);
        self.index_offset += 4;
    }
}
