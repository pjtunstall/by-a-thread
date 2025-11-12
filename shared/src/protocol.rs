use serde::{Deserialize, Serialize};

use crate::maze::Maze;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    ServerTime(f64),
    CountdownStarted { end_time: f64 },
    Welcome { username: String },
    UsernameError { message: String },
    Roster { online: Vec<String> },
    UserJoined { username: String },
    UserLeft { username: String },
    ChatMessage { username: String, content: String },
    ServerInfo { message: String },
    RequestDifficultyChoice,
    GameStarted { maze: Maze },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    SendPasscode(Vec<u8>),
    SetUsername(String),
    SendChat(String),
    RequestStartGame,
    SetDifficulty(u8),
}

pub fn version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}
