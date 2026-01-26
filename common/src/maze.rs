pub mod maker;

use std::fmt;

use glam::{Vec3, vec3};
use serde::{Deserialize, Serialize};

pub use maker::Algorithm;
use maker::MazeMaker;

pub const CELL_SIZE: f32 = 64.0;
pub const RADIUS: usize = 16; // Double and add one to get the width of the maze in grid cells, including edge walls. The reason for this calculation is to ensure an odd number of chars for the width. This lets us draw a nice map with equally thick edges, no matter the value of this parameter used to set its width.

#[derive(Clone, Serialize, Deserialize)]
pub struct Maze {
    pub grid: Vec<Vec<u8>>, // TODO: Consider making this an array of arrays since its size is known and fixed.
    pub spaces: Vec<(usize, usize)>,
}

impl Maze {
    pub fn new(generator: Algorithm) -> Self {
        let maker = MazeMaker::new(RADIUS, RADIUS, generator);
        let grid = maker.grid;
        let mut spaces = Vec::new();

        for (i, row) in grid.iter().enumerate() {
            for (j, &cell) in row.iter().enumerate() {
                if cell == 0 {
                    spaces.push((i, j));
                }
            }
        }

        Self { grid, spaces }
    }

    pub fn is_outside(&self, x: f32, z: f32) -> bool {
        x < 0.0
            || z < 0.0
            || x > self.grid[0].len() as f32 * CELL_SIZE
            || z > self.grid.len() as f32 * CELL_SIZE
    }

    pub fn position_from_grid_coordinates(&self, height: f32, z: usize, x: usize) -> Option<Vec3> {
        if self.spaces.is_empty() {
            None
        } else {
            let position = vec3(
                (x as f32 + 0.5) * CELL_SIZE,
                height,
                (z as f32 + 0.5) * CELL_SIZE,
            );
            Some(position)
        }
    }

    pub fn grid_coordinates_from_position(&self, position: &Vec3) -> Option<(u8, u8)> {
        let grid = &self.grid;
        let col = (position.x / CELL_SIZE).floor() as isize;
        let row = (position.z / CELL_SIZE).floor() as isize;

        if col < 0 || row < 0 {
            return None;
        }

        let col = col as usize;
        let row = row as usize;

        if row >= grid.len() || col >= grid[0].len() {
            return None;
        }

        Some((col as u8, row as u8))
    }

    pub fn log(&self) -> String {
        self.grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&cell| if cell == 0 { "  " } else { "██" })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn is_way_clear(&self, end: &Vec3) -> bool {
        let end_x = (end.x / CELL_SIZE) as usize;
        let end_z = (end.z / CELL_SIZE) as usize;

        let grid = &self.grid;

        let outside_maze =
            end.x < 0.0 || end.z < 0.0 || end_x >= grid[0].len() || end_z >= grid.len();

        outside_maze || grid[end_z][end_x] == 0
    }

    pub fn is_sphere_clear(&self, center: &Vec3, radius: f32) -> bool {
        let grid = &self.grid;
        let grid_width = grid[0].len() as isize;
        let grid_height = grid.len() as isize;

        let min_x = ((center.x - radius) / CELL_SIZE).floor() as isize;
        let max_x = ((center.x + radius) / CELL_SIZE).floor() as isize;
        let min_z = ((center.z - radius) / CELL_SIZE).floor() as isize;
        let max_z = ((center.z + radius) / CELL_SIZE).floor() as isize;

        for z in min_z..=max_z {
            for x in min_x..=max_x {
                if x < 0 || z < 0 || x >= grid_width || z >= grid_height {
                    continue;
                }

                if grid[z as usize][x as usize] == 0 {
                    continue;
                }

                let cell_min_x = x as f32 * CELL_SIZE;
                let cell_max_x = cell_min_x + CELL_SIZE;
                let cell_min_z = z as f32 * CELL_SIZE;
                let cell_max_z = cell_min_z + CELL_SIZE;

                let closest_x = center.x.clamp(cell_min_x, cell_max_x);
                let closest_z = center.z.clamp(cell_min_z, cell_max_z);
                let dx = center.x - closest_x;
                let dz = center.z - closest_z;

                if dx * dx + dz * dz < radius * radius {
                    return false;
                }
            }
        }

        true
    }

    pub fn get_wall_normal(&self, position: Vec3, direction: Vec3, speed: f32) -> Vec3 {
        let previous_position = position - (speed + 0.1) * direction;
        let current_grid_pos = (position / CELL_SIZE).floor();
        let previous_grid_pos = (previous_position / CELL_SIZE).floor();

        let delta = current_grid_pos - previous_grid_pos;

        // If we didn't cross a grid boundary, return the negative direction as fallback.
        if delta.x == 0.0 && delta.z == 0.0 {
            return -direction;
        }

        let is_wall_on_x_side = delta.x != 0.0
            && !self.is_way_clear(&(previous_position + Vec3::new(delta.x * CELL_SIZE, 0.0, 0.0)));

        let is_wall_on_z_side = delta.z != 0.0
            && !self.is_way_clear(&(previous_position + Vec3::new(0.0, 0.0, delta.z * CELL_SIZE)));

        if is_wall_on_x_side && is_wall_on_z_side {
            // Inside corner.
            -direction
        } else if is_wall_on_x_side {
            Vec3::new(-delta.x.signum(), 0.0, 0.0)
        } else if is_wall_on_z_side {
            Vec3::new(0.0, 0.0, -delta.z.signum())
        } else {
            // Outside corner.
            -Vec3::new(delta.x, 0.0, delta.z).normalize()
        }
    }
}

impl fmt::Debug for Maze {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Maze {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.log())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    #[test]
    fn test_backtrack_all_spaces_are_connected() {
        for _ in 0..64 {
            test_all_spaces_are_connected(Algorithm::Backtrack);
        }
    }

    #[test]
    fn test_prim_all_spaces_are_connected() {
        for _ in 0..64 {
            test_all_spaces_are_connected(Algorithm::Prim);
        }
    }

    #[test]
    fn test_wilson_all_spaces_are_connected() {
        for _ in 0..64 {
            test_all_spaces_are_connected(Algorithm::Wilson);
        }
    }

    fn test_all_spaces_are_connected(generator: Algorithm) {
        let maze = Maze::new(generator);
        let grid = &maze.grid;

        let height = grid.len();
        assert!(height != 0, "maze should have some rows");
        let width = grid[0].len();
        assert!(width != 0, "maze should have some columns");

        let mut total_spaces = 0;
        let mut start_pos: Option<(usize, usize)> = None;

        for r in 0..height {
            for c in 0..width {
                if grid[r][c] == 0 {
                    total_spaces += 1;
                    if start_pos.is_none() {
                        start_pos = Some((r, c));
                    }
                }
            }
        }

        let (start_r, start_c) = start_pos.expect("there should be at least one space");
        assert!(total_spaces > 1, "there should be more than one space");

        let mut visited = vec![vec![false; width]; height];
        let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
        let mut visited_count = 0;

        queue.push_back((start_r, start_c));
        visited[start_r][start_c] = true;

        while let Some((r, c)) = queue.pop_front() {
            visited_count += 1;

            let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];

            for (dr, dc) in directions {
                let nr = r as isize + dr;
                let nc = c as isize + dc;

                if nr >= 0 && nr < height as isize && nc >= 0 && nc < width as isize {
                    let nr_u = nr as usize;
                    let nc_u = nc as usize;

                    if grid[nr_u][nc_u] == 0 && !visited[nr_u][nc_u] {
                        visited[nr_u][nc_u] = true;
                        queue.push_back((nr_u, nc_u));
                    }
                }
            }
        }

        assert!(
            total_spaces == visited_count,
            "all spaces should be connected:\n{}",
            maze.log()
        );
    }

    #[test]
    fn test_is_sphere_clear() {
        let grid = vec![vec![1, 1, 1], vec![1, 0, 1], vec![1, 1, 1]];
        let mut spaces = Vec::new();
        for (z, row) in grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if *cell == 0 {
                    spaces.push((z, x));
                }
            }
        }
        let maze = Maze { grid, spaces };

        let center = vec3(1.5 * CELL_SIZE, 0.0, 1.5 * CELL_SIZE);
        assert!(maze.is_sphere_clear(&center, 1.0));
        assert!(!maze.is_sphere_clear(&center, 33.0));

        let outside = vec3(-10.0, 0.0, -10.0);
        assert!(maze.is_sphere_clear(&outside, 5.0));
    }
}
