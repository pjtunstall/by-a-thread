use super::super::{Cell, MazeMaker};

pub trait Wilson {
    fn wilson(&mut self);
}

impl Wilson for MazeMaker {
    fn wilson(&mut self) {
        let mut prospective_cells = self.get_cells();

        let initial_cell = self
            .pick_out_cell(&mut prospective_cells)
            .expect("should be some cells to begin with");
        self.visit_cell(initial_cell);

        let mut finalized_cells = vec![initial_cell];

        while let Some(start_of_walk) = self.pick_out_cell(&mut prospective_cells) {
            walk(
                self,
                start_of_walk,
                &mut finalized_cells,
                &mut prospective_cells,
            );
        }
    }
}

fn walk(
    maze: &mut MazeMaker,
    start_of_walk: Cell,
    finalized_cells: &mut Vec<Cell>,
    prospective_cells: &mut Vec<Cell>,
) {
    let mut walk = vec![start_of_walk];
    let mut curr = start_of_walk;

    while !finalized_cells.contains(&curr) {
        let next = maze
            .pick_neighbor(curr, false, false)
            .expect("there should always be a neighbor unless the grid was malformed");
        if let Some(pos) = walk.iter().position(|&c| c == next) {
            walk.truncate(pos + 1);
        } else {
            walk.push(next);
        }

        curr = next;
    }

    finalize_walk(maze, walk, finalized_cells, prospective_cells);
}

fn finalize_walk(
    maze: &mut MazeMaker,
    walk: Vec<Cell>,
    finalized_cells: &mut Vec<Cell>,
    prospective_cells: &mut Vec<Cell>,
) {
    for i in 0..walk.len() - 1 {
        maze.remove_wall_between(walk[i], walk[i + 1]);
        maze.visit_cell(walk[i]);
        finalized_cells.push(walk[i]);

        if let Some(pos) = prospective_cells.iter().position(|&c| c == walk[i]) {
            prospective_cells.swap_remove(pos);
        }
    }
}
