pub mod algorithms;

use rand::prelude::{IndexedRandom, Rng, ThreadRng};

use algorithms::{backtrack::Backtrack, prim::Prim, wilson::Wilson};

pub enum Algorithm {
    Backtrack, // Easy: more long corridors.
    Wilson,    // Medium: unbiased.
    Prim,      // Hard: more dead-ends.
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
}

impl Cell {
    pub fn new(grid: &[Vec<u8>], x: usize, y: usize) -> Cell {
        debug_assert!(
            x < grid[0].len() || y < grid.len(),
            "cell coordinates are out of bounds"
        );

        Cell { x, y }
    }

    pub fn is_equal(&self, other: &Cell) -> bool {
        self.x == other.x && self.y == other.y
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Wall {
    pub x: usize,
    pub y: usize,
    pub orientation: Orientation,
}

impl Wall {
    fn new(grid: &[Vec<u8>], cell_1: Cell, cell_2: Cell) -> Wall {
        let x = (cell_1.x + cell_2.x) / 2;
        let y = (cell_1.y + cell_2.y) / 2;

        debug_assert!(
            x < grid[0].len() || y < grid.len(),
            "wall coordinates are out of bounds"
        );

        let orientation = if cell_1.x == cell_2.x {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };

        Wall { x, y, orientation }
    }
}

pub struct MazeMaker {
    pub grid: Vec<Vec<u8>>,
    pub rng: ThreadRng,
    width: usize,
    height: usize,
}

impl MazeMaker {
    pub fn new(horizontal_radius: usize, vertical_radius: usize, generator: Algorithm) -> Self {
        let width = 2 * horizontal_radius + 1;
        let height = 2 * vertical_radius + 1;

        let grid = vec![vec![1; width]; height];
        let rng = rand::rng();
        let mut maze = MazeMaker {
            grid,
            width,
            height,
            rng,
        };
        match generator {
            Algorithm::Backtrack => maze.backtrack(),
            Algorithm::Wilson => maze.wilson(),
            Algorithm::Prim => maze.prim(),
        }
        maze
    }

    fn get_neighbors(
        &self,
        cell: Cell,
        only_if_unvisited: bool,
        only_if_visited: bool,
    ) -> Vec<Cell> {
        if only_if_visited && only_if_unvisited {
            return Vec::new();
        }

        let mut valid_neighbors = Vec::new();
        let directions = [(0, 2), (2, 0), (0, -2), (-2, 0)];

        for &(dx, dy) in &directions {
            let nx = cell.x as isize + dx;
            let ny = cell.y as isize + dy;

            let in_bounds =
                nx > 0 && nx < self.width as isize - 1 && ny > 0 && ny < self.height as isize - 1;
            if !in_bounds {
                continue;
            }

            let neighbor = Cell::new(&self.grid, nx as usize, ny as usize);
            let is_visited = self.is_visited(neighbor);

            if only_if_visited || !is_visited || !only_if_unvisited {
                valid_neighbors.push(neighbor);
            }
        }

        valid_neighbors
    }

    fn pick_neighbor(
        &mut self,
        cell: Cell,
        only_if_unvisited: bool,
        only_if_visited: bool,
    ) -> Option<Cell> {
        if only_if_visited && only_if_unvisited {
            return None;
        }

        let neighbors = self.get_neighbors(cell, only_if_unvisited, only_if_visited);

        neighbors.choose(&mut self.rng).copied()
    }

    fn visit_cell(&mut self, cell: Cell) {
        let Cell { x, y } = cell;
        self.grid[y][x] = 0;
    }

    fn is_visited(&self, cell: Cell) -> bool {
        self.grid[cell.y][cell.x] == 0
    }

    fn pick_cell(&mut self) -> Cell {
        let cells = self.get_cells();
        let i = self.rng.random_range(0..cells.len());
        cells[i]
    }

    fn pick_out_cell(&mut self, cells: &mut Vec<Cell>) -> Option<Cell> {
        if cells.is_empty() {
            return None;
        }

        let i = self.rng.random_range(0..cells.len());
        let cell = cells[i];
        cells.remove(i);

        Some(cell)
    }

    fn get_cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();

        for y in 0..self.height {
            for x in 0..self.width {
                if x % 2 == 0 || y % 2 == 0 {
                    continue;
                }
                cells.push(Cell::new(&self.grid, x, y))
            }
        }

        cells
    }

    fn remove_wall_between(&mut self, cell_1: Cell, cell_2: Cell) {
        let x = (cell_1.x + cell_2.x) / 2;
        let y = (cell_1.y + cell_2.y) / 2;
        self.grid[y][x] = 0;
    }
}
