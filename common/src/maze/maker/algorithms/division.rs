use rand::Rng;

use super::super::MazeMaker;

pub trait RecursiveDivision {
    fn recursive_division(&mut self);
}

impl RecursiveDivision for MazeMaker {
    fn recursive_division(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                if y == 0 || y == self.height - 1 || x == 0 || x == self.width - 1 {
                    self.grid[y][x] = 1;
                } else {
                    self.grid[y][x] = 0;
                }
            }
        }

        if self.width > 2 && self.height > 2 {
            self.recursive_divide(1, 1, self.width - 2, self.height - 2);
        }
    }
}

impl MazeMaker {
    fn recursive_divide(&mut self, x: usize, y: usize, width: usize, height: usize) {
        if width < 3 || height < 3 {
            return;
        }

        let horizontal = if width < height {
            true
        } else if height < width {
            false
        } else {
            self.rng.random_bool(0.5)
        };

        if horizontal {
            let range = (height - 1) / 2;
            let wall_y = y + 1 + (self.rng.random_range(0..range) * 2);

            for i in 0..width {
                self.grid[wall_y][x + i] = 1;
            }

            let gap_range = (width + 1) / 2;
            let gap_x = x + (self.rng.random_range(0..gap_range) * 2);
            let gap_x = gap_x.min(x + width - 1);

            self.grid[wall_y][gap_x] = 0;

            self.recursive_divide(x, y, width, wall_y - y);
            self.recursive_divide(x, wall_y + 1, width, y + height - wall_y - 1);
        } else {
            let range = (width - 1) / 2;
            let wall_x = x + 1 + (self.rng.random_range(0..range) * 2);

            for i in 0..height {
                self.grid[y + i][wall_x] = 1;
            }

            let gap_range = (height + 1) / 2;
            let gap_y = y + (self.rng.random_range(0..gap_range) * 2);
            let gap_y = gap_y.min(y + height - 1);

            self.grid[gap_y][wall_x] = 0;

            self.recursive_divide(x, y, wall_x - x, height);
            self.recursive_divide(wall_x + 1, y, x + width - wall_x - 1, height);
        }
    }
}
