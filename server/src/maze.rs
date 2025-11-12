pub mod maker;

pub use maker::Algorithm;
use maker::MazeMaker;

pub const CELL_SIZE: f32 = 64.0;

#[derive(Clone)]
pub struct Maze {
    pub grid: Vec<Vec<u8>>,
    pub spaces: Vec<(usize, usize)>,
}

impl Maze {
    pub fn new(generator: Algorithm, radius: usize) -> Self {
        let maker = MazeMaker::new(radius, radius, generator);
        let grid = maker.grid;
        let mut spaces = Vec::new();

        for (i, row) in grid.iter().enumerate() {
            for (j, &cell) in row.iter().enumerate() {
                if cell == 0 {
                    spaces.push((i, j));
                }
            }
        }

        Self { grid, spaces }
    }

    pub fn log(&self) {
        for row in self.grid.iter() {
            for cell in row {
                if *cell == 0 {
                    print!("  ");
                } else {
                    print!("██");
                }
            }
            println!();
        }
    }
}
