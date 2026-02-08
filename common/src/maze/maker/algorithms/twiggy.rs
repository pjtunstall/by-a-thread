use std::collections::{HashSet, VecDeque};

use rand::Rng;

use super::{super::MazeMaker, GrowthStrategy, Mode};

impl MazeMaker {
    pub fn twiggy(&mut self, mode: Mode, strategy: GrowthStrategy) {
        match mode {
            Mode::Divider => {
                for row in self.grid.iter_mut() {
                    for cell in row.iter_mut() {
                        *cell = 0;
                    }
                }
            }
            Mode::Carver => {
                for row in self.grid.iter_mut() {
                    for cell in row.iter_mut() {
                        *cell = 1;
                    }
                }
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

        let mut initial_space = Vec::new();
        for z in (1..(self.height - 1)).step_by(2) {
            for x in (1..(self.width - 1)).step_by(2) {
                initial_space.push((z, x));
            }
        }

        self.twiggy_divide(initial_space, mode, strategy);
    }

    fn twiggy_divide(&mut self, region: Vec<(usize, usize)>, mode: Mode, strategy: GrowthStrategy) {
        if region.len() <= 1 {
            if mode == Mode::Carver {
                for (z, x) in region {
                    self.grid[z][x] = 0;
                }
            }
            return;
        }

        let rng = &mut self.rng;
        let mut team_a_cells = HashSet::new();
        let mut team_b_cells = HashSet::new();
        let mut frontier_a = VecDeque::new();
        let mut frontier_b = VecDeque::new();

        let seed_index_a = rng.random_range(0..region.len());
        let mut seed_index_b = rng.random_range(0..region.len());
        while seed_index_a == seed_index_b {
            seed_index_b = rng.random_range(0..region.len());
        }

        let seed_a = region[seed_index_a];
        let seed_b = region[seed_index_b];
        team_a_cells.insert(seed_a);
        frontier_a.push_back(seed_a);
        team_b_cells.insert(seed_b);
        frontier_b.push_back(seed_b);

        let mut border_walls = Vec::new();
        let region_set: HashSet<(usize, usize)> = region.iter().cloned().collect();

        while !frontier_a.is_empty() || !frontier_b.is_empty() {
            let use_a = if frontier_a.is_empty() {
                false
            } else if frontier_b.is_empty() {
                true
            } else {
                rng.random_bool(0.5)
            };

            let (active_frontier, my_cells, rival_cells) = if use_a {
                (&mut frontier_a, &mut team_a_cells, &team_b_cells)
            } else {
                (&mut frontier_b, &mut team_b_cells, &team_a_cells)
            };

            let index = match strategy {
                GrowthStrategy::Random => rng.random_range(0..active_frontier.len()),
                GrowthStrategy::Queue => 0,
                GrowthStrategy::Stack => active_frontier.len() - 1,
            };

            let (cz, cx) = active_frontier[index];
            let mut valid_neighbors = Vec::new();

            for (dz, dx) in [(0, 2), (0, -2), (2, 0), (-2, 0)] {
                let nz = (cz as isize + dz) as usize;
                let nx = (cx as isize + dx) as usize;

                if region_set.contains(&(nz, nx)) {
                    if !my_cells.contains(&(nz, nx)) && !rival_cells.contains(&(nz, nx)) {
                        valid_neighbors.push((nz, nx));
                    } else if rival_cells.contains(&(nz, nx)) {
                        // We found the border; make a wall.
                        let wz = (cz as isize + dz / 2) as usize;
                        let wx = (cx as isize + dx / 2) as usize;
                        border_walls.push((wz, wx));
                    }
                }
            }

            if valid_neighbors.is_empty() {
                active_frontier.remove(index);
            } else {
                let (nz, nx) = valid_neighbors[rng.random_range(0..valid_neighbors.len())];
                my_cells.insert((nz, nx));
                active_frontier.push_back((nz, nx));
            }
        }

        if !border_walls.is_empty() {
            if mode == Mode::Divider {
                for (wz, wx) in &border_walls {
                    self.grid[*wz][*wx] = 1;
                }
            }
            let (wz, wx) = border_walls[rng.random_range(0..border_walls.len())];
            self.grid[wz][wx] = 0;
        }

        let next_a: Vec<(usize, usize)> = team_a_cells.into_iter().collect();
        let next_b: Vec<(usize, usize)> = team_b_cells.into_iter().collect();

        for enclave in self.find_enclaves(next_a) {
            self.twiggy_divide(enclave, mode, strategy);
        }
        for enclave in self.find_enclaves(next_b) {
            self.twiggy_divide(enclave, mode, strategy);
        }
    }

    fn find_enclaves(&self, cells: Vec<(usize, usize)>) -> Vec<Vec<(usize, usize)>> {
        let mut unvisited: HashSet<(usize, usize)> = cells.into_iter().collect();
        let mut enclaves = Vec::new();

        while !unvisited.is_empty() {
            // Pick an arbitrary start node.
            let start = *unvisited.iter().next().unwrap();
            unvisited.remove(&start);

            let mut enclave = Vec::new();
            let mut queue = VecDeque::new();

            enclave.push(start);
            queue.push_back(start);

            // Flood fill to find all connected members.
            while let Some((cz, cx)) = queue.pop_front() {
                for (dz, dx) in [(0, 2), (0, -2), (2, 0), (-2, 0)] {
                    let nz = (cz as isize + dz) as usize;
                    let nx = (cx as isize + dx) as usize;

                    // If neighbor is in our unvisited set, it belongs to this
                    // enclave.
                    if unvisited.contains(&(nz, nx)) {
                        unvisited.remove(&(nz, nx));
                        enclave.push((nz, nx));
                        queue.push_back((nz, nx));
                    }
                }
            }
            enclaves.push(enclave);
        }
        enclaves
    }
}
