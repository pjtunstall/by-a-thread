use std::collections::{HashMap, HashSet};

use rand::Rng;

use super::super::MazeMaker;

const MIN_REGION_SIZE: usize = 1;

#[derive(Clone, Copy, PartialEq)]
enum Team {
    A,
    B,
}

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

        for z in 0..self.height {
            for x in 0..self.width {
                let is_border = z == 0 || z == self.height - 1 || x == 0 || x == self.width - 1;
                let is_pillar = z % 2 == 0 && x % 2 == 0;

                if is_border || is_pillar {
                    self.grid[z][x] = 1;
                }
            }
        }

        let mut region = Vec::new();
        for z in (1..(self.height - 1)).step_by(2) {
            for x in (1..(self.width - 1)).step_by(2) {
                region.push((z, x));
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
        let rng = &mut self.rng;

        let seed_index_a = rng.random_range(0..region.len());
        let mut seed_index_b = rng.random_range(0..region.len());
        while seed_index_a == seed_index_b {
            seed_index_b = rng.random_range(0..region.len());
        }

        let seed_a = region[seed_index_a];
        let seed_b = region[seed_index_b];

        let mut labels = HashMap::<(usize, usize), Team>::new();
        let mut unlabeled = region_set.clone();
        let mut frontier = Vec::new();

        labels.insert(seed_a, Team::A);
        labels.insert(seed_b, Team::B);
        unlabeled.remove(&seed_a);
        unlabeled.remove(&seed_b);
        frontier.push((seed_a, Team::A));
        frontier.push((seed_b, Team::B));

        while !unlabeled.is_empty() && !frontier.is_empty() {
            let i = rng.random_range(0..frontier.len());
            let ((cz, cx), team_label) = frontier.swap_remove(i);

            for (dz, dx) in [(0, 2), (0, -2), (2, 0), (-2, 0)] {
                let nz = (cz as isize + dz) as usize;
                let nx = (cx as isize + dx) as usize;

                if !unlabeled.remove(&(nz, nx)) {
                    continue;
                }

                labels.insert((nz, nx), team_label);
                frontier.push(((nz, nx), team_label));
            }
        }

        let mut border_walls = Vec::new();

        for (z, x) in &region {
            let label = labels.get(&(*z, *x)).copied().unwrap_or(Team::A);

            for (dz, dx) in [(0, 2), (2, 0)] {
                let nz = (*z as isize + dz) as usize;
                let nx = (*x as isize + dx) as usize;

                if !region_set.contains(&(nz, nx)) {
                    continue;
                }

                let neighbor_label = labels.get(&(nz, nx)).copied().unwrap_or(Team::A);

                if label != neighbor_label {
                    let wz = (*z + nz) / 2;
                    let wx = (*x + nx) / 2;

                    self.grid[wz][wx] = 1;

                    border_walls.push((wz, wx));
                }
            }
        }

        if !border_walls.is_empty() {
            let gap_index = rng.random_range(0..border_walls.len());
            let (wz, wx) = border_walls[gap_index];
            self.grid[wz][wx] = 0;
        }

        let mut region_a = Vec::new();
        let mut region_b = Vec::new();

        for (z, x) in region {
            let label = labels.get(&(z, x)).copied().unwrap_or(Team::A);
            match label {
                Team::A => region_a.push((z, x)),
                Team::B => region_b.push((z, x)),
            }
        }

        self.blobby_divide(region_a);
        self.blobby_divide(region_b);
    }
}
