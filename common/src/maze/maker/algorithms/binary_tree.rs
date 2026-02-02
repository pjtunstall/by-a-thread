use rand::random_range;

use super::super::MazeMaker;

pub trait BinaryTree {
    fn binary_tree(&mut self);
}

impl BinaryTree for MazeMaker {
    fn binary_tree(&mut self) {
        // Calculate the spine coordinates, exnsuring that they're odd so that
        // they don't include pillars.
        let mid_x = (self.width / 2) | 1;
        let mid_y = (self.height / 2) | 1;

        for y in (1..self.height - 1).rev() {
            for x in 1..self.width - 1 {
                // Only consider rooms, not walls or pillars.
                if x % 2 == 0 || y % 2 == 0 {
                    continue;
                }

                self.grid[y][x] = 0;

                let mut directions = Vec::new();

                // Horizontal bias: move towards `mid_x`. If we're at 'mid_x`,
                // don't add horizontal options. This prevents the left and
                // right sides from looping into each other.
                if x < mid_x {
                    directions.push((0, 1)); // East
                } else if x > mid_x {
                    directions.push((0, -1)); // West
                }

                // Vertical bias: move towards `mid_y`. If we're at `mid_y`,
                // don't add vertical options.
                if y < mid_y {
                    directions.push((1, 0)); // South
                } else if y > mid_y {
                    directions.push((-1, 0)); // North
                }

                if !directions.is_empty() {
                    let r = random_range(0..directions.len());
                    let (dy, dx) = directions[r];
                    let wall_y = (y as isize + dy) as usize;
                    let wall_x = (x as isize + dx) as usize;
                    self.grid[wall_y][wall_x] = 0;
                }
            }
        }
    }
}
