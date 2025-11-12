use std::collections::HashSet;

use rand::prelude::IteratorRandom;

use super::super::{Cell, MazeMaker, Orientation, Wall};

pub trait Prim {
    fn prim(&mut self);
}

impl Prim for MazeMaker {
    fn prim(&mut self) {
        let initial_cell = self.pick_cell();
        self.visit_cell(initial_cell);

        let mut frontier = HashSet::new();
        add_walls(self, initial_cell, &mut frontier);

        while let Some(wall) = pick_wall(self, &frontier) {
            let (cell_1, cell_2) = get_flanking_cells(self, wall);

            let is_visited_1 = self.is_visited(cell_1);
            let is_visited_2 = self.is_visited(cell_2);

            if is_visited_1 != is_visited_2 {
                self.remove_wall_between(cell_1, cell_2);
                let new_cell = if is_visited_1 { cell_2 } else { cell_1 };
                visit_new_cell_and_add_its_walls(self, new_cell, &mut frontier);
            }

            frontier.remove(&wall);
        }
    }
}

fn is_there_a_wall_between(maze: &MazeMaker, cell_1: Cell, cell_2: Cell) -> (bool, Wall) {
    let wall = Wall::new(&maze.grid, cell_1, cell_2);
    (!is_wall_clear(maze, wall), wall)
}

fn is_wall_clear(maze: &MazeMaker, wall: Wall) -> bool {
    let Wall { x, y, .. } = wall;
    maze.grid[y][x] == 0
}

fn visit_new_cell_and_add_its_walls(
    maze: &mut MazeMaker,
    cell: Cell,
    frontier: &mut HashSet<Wall>,
) {
    maze.visit_cell(cell);
    add_walls(maze, cell, frontier);
}

fn get_flanking_cells(maze: &MazeMaker, wall: Wall) -> (Cell, Cell) {
    if wall.orientation == Orientation::Horizontal {
        (
            Cell::new(&maze.grid, wall.x - 1, wall.y),
            Cell::new(&maze.grid, wall.x + 1, wall.y),
        )
    } else {
        (
            Cell::new(&maze.grid, wall.x, wall.y - 1),
            Cell::new(&maze.grid, wall.x, wall.y + 1),
        )
    }
}

fn add_walls(maze: &mut MazeMaker, cell: Cell, frontier: &mut HashSet<Wall>) {
    let neighbors = maze.get_neighbors(cell, false, false);
    for neighbor in neighbors {
        let (is_there_a_wall, wall) = is_there_a_wall_between(maze, cell, neighbor);
        if is_there_a_wall {
            frontier.insert(wall);
        }
    }
}

fn pick_wall(maze: &mut MazeMaker, frontier: &HashSet<Wall>) -> Option<Wall> {
    frontier.iter().choose(&mut maze.rng).copied()
}
