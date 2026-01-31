pub mod after_game;
pub mod initialize;
pub mod update;

pub use initialize::{MapOverlay, initialize_map};

pub const WALL_SYMBOL: &str = "█";
pub const SPACE_SYMBOL: &str = " ";
