use serde::{Deserialize, Serialize};

use crate::{player::PlayerInput, snapshot::InitialData};

pub const GAME_ALREADY_STARTED_MESSAGE: &str =
    "The game is already in progress. Please try again after this match.";

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    ServerTime(f64),
    CountdownStarted {
        end_time: f64,
        game_data: InitialData,
    },
    Welcome {
        username: String,
    },
    UsernameError {
        message: String,
    },
    AppointHost,
    Roster {
        online: Vec<String>,
    },
    UserJoined {
        username: String,
    },
    UserLeft {
        username: String,
    },
    ChatMessage {
        username: String,
        content: String,
    },
    ServerInfo {
        message: String,
    },
    BeginDifficultySelection, // Allow host to move to phase where they choose a difficulty.
    DenyDifficultySelection,  // Refuse non-host client who asks to choose a difficulty level.
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ClientMessage {
    SendPasscode(Vec<u8>),
    SetUsername(String),
    SendChat(String),
    RequestStartGame,
    SetDifficulty(u8),
    Input(PlayerInput),
}

pub fn version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}
