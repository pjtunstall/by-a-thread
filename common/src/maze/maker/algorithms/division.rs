use rand::Rng;

use super::super::MazeMaker;

pub trait RecursiveDivision {
    fn recursive_division(&mut self);
}

impl RecursiveDivision for MazeMaker {
    fn recursive_division(&mut self) {
        for z in 0..self.height {
            for x in 0..self.width {
                if z == 0 || z == self.height - 1 || x == 0 || x == self.width - 1 {
                    self.grid[z][x] = 1;
                } else {
                    self.grid[z][x] = 0;
                }
            }
        }

        if self.width > 2 && self.height > 2 {
            self.recursive_divide(1, 1, self.width - 2, self.height - 2);
        }
    }
}

impl MazeMaker {
    fn recursive_divide(&mut self, x: usize, z: usize, width: usize, height: usize) {
        if width < 3 || height < 3 {
            return;
        }

        let rng = &mut self.rng;

        let horizontal = if width < height {
            true
        } else if height < width {
            false
        } else {
            rng.random_bool(0.5)
        };

        if horizontal {
            let range = (height - 1) / 2;
            let wall_z = z + 1 + (rng.random_range(0..range) * 2);

            for i in 0..width {
                self.grid[wall_z][x + i] = 1;
            }

            let gap_range = (width + 1) / 2;
            let gap_x = x + (rng.random_range(0..gap_range) * 2);
            let gap_x = gap_x.min(x + width - 1);

            self.grid[wall_z][gap_x] = 0;

            self.recursive_divide(x, z, width, wall_z - z);
            self.recursive_divide(x, wall_z + 1, width, z + height - wall_z - 1);
        } else {
            let range = (width - 1) / 2;
            let wall_x = x + 1 + (rng.random_range(0..range) * 2);

            for i in 0..height {
                self.grid[z + i][wall_x] = 1;
            }

            let gap_range = (height + 1) / 2;
            let gap_z = z + (rng.random_range(0..gap_range) * 2);
            let gap_z = gap_z.min(z + height - 1);

            self.grid[gap_z][wall_x] = 0;

            self.recursive_divide(x, z, wall_x - x, height);
            self.recursive_divide(wall_x + 1, z, x + width - wall_x - 1, height);
        }
    }
}
