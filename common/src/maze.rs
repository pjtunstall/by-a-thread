pub mod maker;

use std::fmt;

use glam::{Vec3, vec3};
use rand;
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

    pub fn make_exit(&mut self, solo_player_grid_coords: (usize, usize)) -> (usize, usize) {
        let grid = &self.grid;
        let height = grid.len();
        let width = if height > 0 { grid[0].len() } else { 0 };

        let mut candidates = Vec::new();

        let has_space_neighbor = |z: usize, x: usize| {
            (z > 0 && grid[z - 1][x] == 0)
                || (z + 1 < height && grid[z + 1][x] == 0)
                || (x > 0 && grid[z][x - 1] == 0)
                || (x + 1 < width && grid[z][x + 1] == 0)
        };

        let mut check_if_valid = |z, x| {
            if has_space_neighbor(z, x) {
                candidates.push((z, x));
            }
        };

        let row = solo_player_grid_coords.0;
        let col = solo_player_grid_coords.1;

        let (vertical_dist, horizontal_dist) = Self::distance_from_center(height, width, row, col);
        let is_player_more_vertical = vertical_dist > horizontal_dist;

        let fallback = if is_player_more_vertical {
            let fallback = Self::vertical_fallback(height, width, row, col);
            self.collect_vertical_edge_candidates(fallback.0, width, &mut check_if_valid);
            fallback
        } else {
            let fallback = Self::horizontal_fallback(height, width, row, col);
            self.collect_horizontal_edge_candidates(height, fallback.1, &mut check_if_valid);
            fallback
        };

        if candidates.is_empty() {
            self.punch_path_to_nearest_space_bfs(fallback)
        } else {
            let i = rand::random_range(0..candidates.len());
            let exit = candidates[i];
            self.grid[exit.0][exit.1] = 0;
            self.spaces.push(exit);
            exit
        }
    }

    fn distance_from_center(height: usize, width: usize, row: usize, col: usize) -> (usize, usize) {
        let center_row = height / 2;
        let center_col = width / 2;

        let vertical_dist = if row > center_row {
            row - center_row
        } else {
            center_row - row
        };

        let horizontal_dist = if col > center_col {
            col - center_col
        } else {
            center_col - col
        };

        (vertical_dist, horizontal_dist)
    }

    fn vertical_fallback(height: usize, width: usize, row: usize, col: usize) -> (usize, usize) {
        let center_row = height / 2;
        let player_in_top_half = row < center_row;
        let edge_z = if player_in_top_half {
            height.saturating_sub(1)
        } else {
            0
        };

        if width <= 2 {
            return (edge_z, 0);
        }

        let fallback_x = col.min(width.saturating_sub(2)).max(1);
        (edge_z, fallback_x)
    }

    fn horizontal_fallback(height: usize, width: usize, row: usize, col: usize) -> (usize, usize) {
        let center_col = width / 2;
        let player_in_left_half = col < center_col;
        let edge_x = if player_in_left_half {
            width.saturating_sub(1)
        } else {
            0
        };

        if height <= 2 {
            return (0, edge_x);
        }

        let fallback_z = row.min(height.saturating_sub(2)).max(1);
        (fallback_z, edge_x)
    }

    fn collect_vertical_edge_candidates(
        &self,
        edge_z: usize,
        width: usize,
        check_if_valid: &mut impl FnMut(usize, usize),
    ) {
        if width <= 2 {
            return;
        }

        for x in 1..width.saturating_sub(1) {
            check_if_valid(edge_z, x);
        }
    }

    fn collect_horizontal_edge_candidates(
        &self,
        height: usize,
        edge_x: usize,
        check_if_valid: &mut impl FnMut(usize, usize),
    ) {
        if height <= 2 {
            return;
        }

        for z in 1..height.saturating_sub(1) {
            check_if_valid(z, edge_x);
        }
    }

    fn punch_path_to_nearest_space_bfs(&mut self, exit_coords: (usize, usize)) -> (usize, usize) {
        let height = self.grid.len();
        if height == 0 {
            return exit_coords;
        }
        let width = self.grid[0].len();
        if width == 0 {
            return exit_coords;
        }

        let (start_z, start_x) = exit_coords;
        if start_z >= height || start_x >= width {
            return exit_coords;
        }

        let mut visited = vec![vec![false; width]; height];
        let mut prev = vec![vec![None; width]; height];
        let mut queue = std::collections::VecDeque::new();

        visited[start_z][start_x] = true;
        queue.push_back((start_z, start_x));

        let mut target = None;

        while let Some((z, x)) = queue.pop_front() {
            if self.grid[z][x] == 0 && (z, x) != exit_coords {
                target = Some((z, x));
                break;
            }

            let neighbors = [
                (z.wrapping_add(1), x),
                (z.wrapping_sub(1), x),
                (z, x.wrapping_add(1)),
                (z, x.wrapping_sub(1)),
            ];

            for (nz, nx) in neighbors {
                if nz >= height || nx >= width {
                    continue;
                }
                if visited[nz][nx] {
                    continue;
                }
                visited[nz][nx] = true;
                prev[nz][nx] = Some((z, x));
                queue.push_back((nz, nx));
            }
        }

        let mut current = target.unwrap_or(exit_coords);

        loop {
            let (z, x) = current;
            if self.grid[z][x] != 0 {
                self.grid[z][x] = 0;
                self.spaces.push((z, x));
            }
            if current == exit_coords {
                break;
            }
            if let Some(p) = prev[z][x] {
                current = p;
            } else {
                break;
            }
        }

        exit_coords
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
            let maze = Maze::new(Algorithm::Backtrack);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_prim_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::Prim);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_wilson_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::Wilson);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_blobby_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::Blobby);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_recursive_division_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::RecursiveDivision);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_binary_tree_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::BinaryTree);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_kruskal_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::Kruskal);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_voronoi_stack_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::VoronoiStack);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_voronoi_random_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::VoronoiRandom);
            assert_all_spaces_are_connected(&maze);
        }
    }

    #[test]
    fn test_voronoi_queue_all_spaces_are_connected() {
        for _ in 0..64 {
            let maze = Maze::new(Algorithm::VoronoiQueue);
            assert_all_spaces_are_connected(&maze);
        }
    }

    fn assert_all_spaces_are_connected(maze: &Maze) {
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

        assert!(
            total_spaces == maze.spaces.len(),
            "total spaces should equal `maze.spaces.len()`, got {} and {}",
            total_spaces,
            maze.spaces.len()
        );

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
    fn test_make_exit_picks_candidate_on_selected_edge_and_punches_to_interior_space() {
        let grid = vec![
            vec![1, 1, 1, 1, 1, 1, 1],
            vec![1, 1, 1, 0, 1, 1, 1], // Player at (1, 3).
            vec![1, 1, 1, 0, 1, 1, 1],
            vec![1, 1, 0, 0, 0, 1, 1],
            vec![1, 1, 1, 0, 1, 1, 1],
            vec![1, 1, 1, 0, 1, 1, 1],
            vec![1, 1, 1, 1, 1, 1, 1],
        ];

        let mut spaces = Vec::new();
        for (z, row) in grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if *cell == 0 {
                    spaces.push((z, x));
                }
            }
        }

        let mut maze = Maze { grid, spaces };

        let solo_player_grid_coords = (1, 3);
        let exit = maze.make_exit(solo_player_grid_coords);

        assert_eq!(exit, (6, 3));
        assert_eq!(maze.grid[6][3], 0);
        assert_eq!(maze.grid[5][3], 0);
        assert_all_spaces_are_connected(&maze);
    }

    #[test]
    fn test_make_exit_uses_fallback_when_only_other_edge_has_candidate() {
        let grid = vec![
            vec![1, 1, 1, 1, 1, 1, 1],
            vec![1, 1, 1, 0, 1, 1, 1], // Player at (1, 3).
            vec![1, 1, 1, 0, 1, 1, 1],
            vec![1, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 1, 0, 1, 1, 1],
            vec![1, 1, 1, 1, 1, 1, 1],
            vec![1, 1, 1, 1, 1, 1, 1],
        ];

        let mut spaces = Vec::new();
        for (z, row) in grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if *cell == 0 {
                    spaces.push((z, x));
                }
            }
        }

        let mut maze = Maze { grid, spaces };

        let solo_player_grid_coords = (1, 3);
        let exit = maze.make_exit(solo_player_grid_coords);

        assert_eq!(exit, (6, 3));
        assert_eq!(maze.grid[6][3], 0);
        assert_eq!(maze.grid[5][3], 0);
        assert_all_spaces_are_connected(&maze);
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
