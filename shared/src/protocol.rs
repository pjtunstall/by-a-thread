use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    // Sent 10-20x/sec on an unreliable channel.
    // Contains the server's time in seconds (f64).
    ServerTime(f64),

    // Sent once on a reliable channel to start the countdown.
    // Contains the exact server time (f64) when the countdown will end.
    CountdownStarted { end_time: f64 },
    Welcome { username: String },
    UsernameError { message: String },
    Roster { online: Vec<String> },
    UserJoined { username: String },
    UserLeft { username: String },
    ChatMessage { username: String, content: String },

    // E.g. "Authentication successful!" or "Incorrect passcode..."
    ServerInfo { message: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    SendPasscode(Vec<u8>),
    SetUsername(String),
    SendChat(String),
    RequestStartGame,
}

pub fn version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}
