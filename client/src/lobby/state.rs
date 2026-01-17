use macroquad::prelude::*;

use crate::game::world::maze::MazeMeshes;
use common::{auth::Passcode, snapshot::InitialData};

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
        sky_mesh: Mesh,
    },
    ChoosingDifficulty {
        prompt_printed: bool,
        choice_sent: bool,
    },
}

impl std::fmt::Debug for Lobby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lobby::ServerAddress { prompt_printed } => f
                .debug_struct("ServerAddress")
                .field("prompt_printed", prompt_printed)
                .finish(),
            Lobby::Passcode { prompt_printed } => f
                .debug_struct("Passcode")
                .field("prompt_printed", prompt_printed)
                .finish(),
            Lobby::Connecting { pending_passcode } => f
                .debug_struct("Connecting")
                .field("pending_passcode", pending_passcode)
                .finish(),
            Lobby::Authenticating {
                waiting_for_input,
                guesses_left,
                waiting_for_server,
            } => f
                .debug_struct("Authenticating")
                .field("waiting_for_input", waiting_for_input)
                .field("guesses_left", guesses_left)
                .field("waiting_for_server", waiting_for_server)
                .finish(),
            Lobby::ChoosingUsername { prompt_printed } => f
                .debug_struct("ChoosingUsername")
                .field("prompt_printed", prompt_printed)
                .finish(),
            Lobby::AwaitingUsernameConfirmation => {
                write!(f, "AwaitingUsernameConfirmation")
            }
            Lobby::Chat {
                awaiting_initial_roster,
                waiting_for_server,
            } => f
                .debug_struct("Chat")
                .field("awaiting_initial_roster", awaiting_initial_roster)
                .field("waiting_for_server", waiting_for_server)
                .finish(),
            Lobby::Countdown {
                end_time,
                game_data,
                maze_meshes,
                sky_mesh: _,
            } => f
                .debug_struct("Countdown")
                .field("end_time", end_time)
                .field("game_data", game_data)
                .field("maze_meshes", maze_meshes)
                .field("sky_mesh", &"<Mesh>")
                .finish(),
            Lobby::ChoosingDifficulty {
                prompt_printed,
                choice_sent,
            } => f
                .debug_struct("ChoosingDifficulty")
                .field("prompt_printed", prompt_printed)
                .field("choice_sent", choice_sent)
                .finish(),
        }
    }
}
