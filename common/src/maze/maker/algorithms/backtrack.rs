use super::super::MazeMaker;

pub trait Backtrack {
    fn backtrack(&mut self);
}

impl Backtrack for MazeMaker {
    fn backtrack(&mut self) {
        let mut stack = Vec::new();

        let mut cells = self.get_cells();
        let initial_cell = self
            .pick_out_cell(&mut cells)
            .expect("should always be some cells unless the grid was malformed");
        stack.push(initial_cell);

        while let Some(curr) = stack.pop() {
            if let Some(next) = self.pick_neighbor(curr, true, false) {
                stack.push(curr);
                self.remove_wall_between(curr, next);
                self.visit_cell(next);
                stack.push(next);
            }
        }
    }
}
