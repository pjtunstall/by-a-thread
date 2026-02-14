pub mod create;
pub mod join;

use serde::Serialize;

pub use create::create_game;
pub use join::join_game;

#[derive(Serialize)]
pub struct ErrorBody {
    kind: &'static str,
    message: &'static str,
}
