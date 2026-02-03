use std::collections::{HashMap, HashSet};

use rand::Rng;

use super::super::MazeMaker;

const MIN_REGION_SIZE: usize = 1;

pub trait Blobby {
    fn blobby(&mut self);
}

impl Blobby for MazeMaker {
    fn blobby(&mut self) {
        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = 0;
            }
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let is_border = y == 0 || y == self.height - 1 || x == 0 || x == self.width - 1;
                let is_pillar = y % 2 == 0 && x % 2 == 0;

                if is_border || is_pillar {
                    self.grid[y][x] = 1;
                }
            }
        }

        let mut region = Vec::new();
        for y in (1..self.height).step_by(2) {
            for x in (1..self.width).step_by(2) {
                region.push((y, x));
            }
        }

        self.blobby_divide(region);
    }
}

impl MazeMaker {
    fn blobby_divide(&mut self, region: Vec<(usize, usize)>) {
        if region.len() <= MIN_REGION_SIZE {
            return;
        }

        let region_set: HashSet<(usize, usize)> = region.iter().copied().collect();

        let seed_index_a = self.rng.random_range(0..region.len());
        let mut seed_index_b = self.rng.random_range(0..region.len());
        while seed_index_a == seed_index_b {
            seed_index_b = self.rng.random_range(0..region.len());
        }

        let seed_a = region[seed_index_a];
        let seed_b = region[seed_index_b];

        let mut labels = HashMap::<(usize, usize), bool>::new();
        let mut unlabeled = region_set.clone();
        let mut frontier = Vec::new();

        labels.insert(seed_a, true);
        labels.insert(seed_b, false);
        unlabeled.remove(&seed_a);
        unlabeled.remove(&seed_b);
        frontier.push((seed_a, true));
        frontier.push((seed_b, false));

        while !unlabeled.is_empty() && !frontier.is_empty() {
            let i = self.rng.random_range(0..frontier.len());
            let ((cy, cx), is_a) = frontier.swap_remove(i);

            for (dy, dx) in [(0, 2), (0, -2), (2, 0), (-2, 0)] {
                let ny = (cy as isize + dy) as usize;
                let nx = (cx as isize + dx) as usize;

                if !unlabeled.remove(&(ny, nx)) {
                    continue;
                }

                labels.insert((ny, nx), is_a);
                frontier.push(((ny, nx), is_a));
            }
        }

        let mut border_walls = Vec::new();

        for (y, x) in &region {
            let label = labels.get(&(*y, *x)).copied().unwrap_or(true);

            for (dy, dx) in [(0, 2), (2, 0)] {
                let ny = (*y as isize + dy) as usize;
                let nx = (*x as isize + dx) as usize;

                if !region_set.contains(&(ny, nx)) {
                    continue;
                }

                let neighbor_label = labels.get(&(ny, nx)).copied().unwrap_or(true);

                if label != neighbor_label {
                    let wy = (*y + ny) / 2;
                    let wx = (*x + nx) / 2;

                    self.grid[wy][wx] = 1;

                    border_walls.push((wy, wx));
                }
            }
        }

        if !border_walls.is_empty() {
            let gap_index = self.rng.random_range(0..border_walls.len());
            let (wy, wx) = border_walls[gap_index];
            self.grid[wy][wx] = 0;
        }

        let mut region_a = Vec::new();
        let mut region_b = Vec::new();

        for (y, x) in region {
            if labels.get(&(y, x)).copied().unwrap_or(true) {
                region_a.push((y, x));
            } else {
                region_b.push((y, x));
            }
        }

        self.blobby_divide(region_a);
        self.blobby_divide(region_b);
    }
}
