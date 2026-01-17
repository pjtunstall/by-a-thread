use macroquad::prelude::*;

use crate::game::world::maze::MazeMeshes;
use crate::game::world::sky::SkyMesh;
use common::{auth::Passcode, snapshot::InitialData};

#[derive(Debug)]
pub enum Lobby {
    ServerAddress {
        prompt_printed: bool,
    },
    Passcode {
        prompt_printed: bool,
    },
    Connecting {
        pending_passcode: Option<Passcode>,
    },
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
        waiting_for_server: bool,
    },
    ChoosingUsername {
        prompt_printed: bool,
    },
    AwaitingUsernameConfirmation,
    Chat {
        awaiting_initial_roster: bool,
        waiting_for_server: bool,
    },
    Countdown {
        end_time: f64,
        game_data: InitialData,
        maze_meshes: Option<MazeMeshes>,
        sky_mesh: SkyMesh,
    },
    ChoosingDifficulty {
        prompt_printed: bool,
        choice_sent: bool,
    },
}
