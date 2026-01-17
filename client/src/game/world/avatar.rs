use std::f32::consts::PI;

use macroquad::prelude::*;

pub struct OrientedSphereMesh {
    base_vertices: Vec<(Vec3, Vec2)>,
    mesh: Mesh,
}

impl OrientedSphereMesh {
    pub fn new() -> Self {
        const RINGS: usize = 16;
        const SLICES: usize = 16;

        let triangle_count = (RINGS + 1) * SLICES * 2;
        let mut base_vertices = Vec::with_capacity(triangle_count * 3);

        let mut push_triangle = |v1: Vec3, uv1: Vec2, v2: Vec3, uv2: Vec2, v3: Vec3, uv3: Vec2| {
            base_vertices.push((v1, uv1));
            base_vertices.push((v2, uv2));
            base_vertices.push((v3, uv3));
        };

        use std::f32::consts::PI;
        let pi34 = PI / 2. * 3.;
        let pi2 = PI * 2.;
        let rings = RINGS as f32;
        let slices = SLICES as f32;

        for i in 0..RINGS + 1 {
            for j in 0..SLICES {
                let i = i as f32;
                let j = j as f32;

                let v1 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).cos(),
                );
                let uv1 = vec2(i / rings, j / slices);
                let v2 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv2 = vec2((i + 1.) / rings, (j + 1.) / slices);
                let v3 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * (j * pi2 / slices).cos(),
                );
                let uv3 = vec2((i + 1.) / rings, j / slices);
                push_triangle(v1, uv1, v2, uv2, v3, uv3);

                let v1 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).cos(),
                );
                let uv1 = vec2(i / rings, j / slices);
                let v2 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv2 = vec2(i / rings, (j + 1.) / slices);
                let v3 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv3 = vec2((i + 1.) / rings, (j + 1.) / slices);
                push_triangle(v1, uv1, v2, uv2, v3, uv3);
            }
        }

        let vertices = base_vertices
            .iter()
            .map(|(position, uv)| Vertex::new2(*position, *uv, WHITE))
            .collect();
        let indices = (0..base_vertices.len() as u16).collect();

        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };

        Self {
            base_vertices,
            mesh,
        }
    }

    pub fn draw(
        &mut self,
        center: Vec3,
        radius: f32,
        texture: Option<&Texture2D>,
        color: Color,
        yaw: f32,
        pitch: f32,
    ) {
        let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        let scale = vec3(radius, radius, radius);
        let color_bytes: [u8; 4] = color.into();

        for (vertex, (base_position, uv)) in
            self.mesh.vertices.iter_mut().zip(self.base_vertices.iter())
        {
            vertex.position = rotation.mul_vec3(*base_position * scale) + center;
            vertex.uv = *uv;
            vertex.color = color_bytes;
        }

        self.mesh.texture = texture.cloned();
        draw_mesh(&self.mesh);
    }
}

pub struct DiskMesh {
    base_vertices: Vec<(Vec3, Vec2)>,
    mesh: Mesh,
}

impl DiskMesh {
    pub fn new() -> Self {
        const SLICES: usize = 24;

        let triangle_count = SLICES;
        let mut base_vertices = Vec::with_capacity(triangle_count * 3);

        let two_pi = PI * 2.0;

        for i in 0..SLICES {
            let angle_a = (i as f32) * two_pi / (SLICES as f32);
            let angle_b = ((i + 1) as f32) * two_pi / (SLICES as f32);

            let v1 = vec3(angle_a.cos(), 0.0, angle_a.sin());
            let v2 = vec3(angle_b.cos(), 0.0, angle_b.sin());

            base_vertices.push((Vec3::ZERO, vec2(0.5, 0.5)));
            base_vertices.push((
                v1,
                vec2(0.5 + 0.5 * angle_a.cos(), 0.5 + 0.5 * angle_a.sin()),
            ));
            base_vertices.push((
                v2,
                vec2(0.5 + 0.5 * angle_b.cos(), 0.5 + 0.5 * angle_b.sin()),
            ));
        }

        let vertices = base_vertices
            .iter()
            .map(|(position, uv)| Vertex::new2(*position, *uv, WHITE))
            .collect();
        let indices = (0..base_vertices.len() as u16).collect();

        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };

        Self {
            base_vertices,
            mesh,
        }
    }

    pub fn draw(&mut self, center: Vec3, radius: f32, color: Color) {
        let scale = vec3(radius, 1.0, radius);
        let color_bytes: [u8; 4] = color.into();

        for (vertex, (base_position, uv)) in
            self.mesh.vertices.iter_mut().zip(self.base_vertices.iter())
        {
            vertex.position = *base_position * scale + center;
            vertex.uv = *uv;
            vertex.color = color_bytes;
        }

        self.mesh.texture = None;
        draw_mesh(&self.mesh);
    }
}
