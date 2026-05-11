pub mod backtracker;
pub mod binary_tree;
pub mod blobby;
pub mod division;
pub mod kruskal;
pub mod prim;
pub mod twiggy;
pub mod wilson;

#[derive(Clone, Copy)]
pub enum GrowthStrategy {
    Random,
    Queue,
    Stack,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Divider,
    Carver,
}
