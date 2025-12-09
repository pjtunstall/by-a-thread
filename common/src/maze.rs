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
    pub grid: Vec<Vec<u8>>,
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

pub fn which_way_to_turn(old_position: &Vec3, contact_point: &Vec3) -> f32 {
    let grid_old_x = (old_position.x / CELL_SIZE) as usize;
    let grid_old_z = (old_position.z / CELL_SIZE) as usize;
    let grid_new_x = (contact_point.x / CELL_SIZE) as usize;
    let grid_new_z = (contact_point.z / CELL_SIZE) as usize;

    if grid_new_z < grid_old_z {
        if old_position.x < contact_point.x {
            return -1.0;
        } else {
            return 1.0;
        }
    }

    if grid_new_z > grid_old_z {
        if old_position.x < contact_point.x {
            return 1.0;
        } else {
            return -1.0;
        }
    }

    if grid_new_x < grid_old_x {
        if old_position.z < contact_point.z {
            return 1.0;
        } else {
            return -1.0;
        }
    }

    if grid_new_x > grid_old_x {
        if old_position.z < contact_point.z {
            return -1.0;
        } else {
            return 1.0;
        }
    }

    0.0
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
}
