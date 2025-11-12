use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    /// Sent 10-20x/sec on an unreliable channel.
    /// Contains the server's time in seconds (f64).
    ServerTime(f64),

    /// Sent once on a reliable channel to start the countdown.
    /// Contains the exact server time (f64) when the countdown will end.
    CountdownStarted { end_time: f64 },
    // Add other messages here later, e.g.:
    // ChatMessage(String),
    // PlayerJoined(String),
}

pub fn version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}
