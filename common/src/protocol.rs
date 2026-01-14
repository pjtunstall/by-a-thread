use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{
    player::{Color, PlayerInput},
    ring::WireItem,
    snapshot::{InitialData, Snapshot},
};

pub const AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE: &str =
    "Incorrect passcode. Disconnecting.";
pub const AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE: &str = "Incorrect passcode. Try again.";
pub const GAME_ALREADY_STARTED_MESSAGE: &str =
    "The game is already in progress. Please try again after this match.";

pub fn auth_success_message(max_username_length: usize) -> String {
    format!(
        "Authentication successful! Please enter a username (1-{} characters).",
        max_username_length
    )
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterGameExitReason {
    Disconnected,
    Slain,
    Winner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AfterGameLeaderboardEntry {
    pub username: String,
    pub color: Color,
    pub ticks_survived: u64,
    pub exit_reason: AfterGameExitReason,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerRosterEntry {
    pub username: String,
    pub color: Color,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Snapshot(WireItem<Snapshot>),
    BulletEvent(BulletEvent),
    ServerTime(f64),
    CountdownStarted {
        end_time: f64,
        game_data: InitialData,
    },
    Welcome {
        username: String,
        color: Color,
    },
    UsernameError {
        message: String,
    },
    AppointHost,
    Roster {
        online: Vec<PlayerRosterEntry>,
    },
    UserJoined {
        username: String,
    },
    UserLeft {
        username: String,
    },
    ChatMessage {
        username: String,
        color: Color,
        content: String,
    },
    AfterGameRoster {
        hades_shades: Vec<PlayerRosterEntry>,
    },
    AfterGameLeaderboard {
        entries: Vec<AfterGameLeaderboardEntry>,
    },
    ServerInfo {
        message: String,
    },
    BeginDifficultySelection, // Allow host to move to phase where they choose a difficulty.
    DenyDifficultySelection,  // Refuse non-host client who asks to choose a difficulty level.
}

impl ServerMessage {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Snapshot(_) => "Snapshot",
            Self::BulletEvent(_) => "BulletEvent",
            Self::ServerTime(_) => "ServerTime",
            Self::CountdownStarted { .. } => "CountdownStarted",
            Self::Welcome { .. } => "Welcome",
            Self::UsernameError { .. } => "UsernameError",
            Self::AppointHost => "AppointHost",
            Self::Roster { .. } => "Roster",
            Self::UserJoined { .. } => "UserJoined",
            Self::UserLeft { .. } => "UserLeft",
            Self::ChatMessage { .. } => "ChatMessage",
            Self::AfterGameRoster { .. } => "AfterGameRoster",
            Self::AfterGameLeaderboard { .. } => "AfterGameLeaderboard",
            Self::ServerInfo { .. } => "ServerInfo",
            Self::BeginDifficultySelection => "BeginDifficultySelection",
            Self::DenyDifficultySelection => "DenyDifficultySelection",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum BulletEvent {
    Spawn {
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
        fire_nonce: Option<u32>,
        shooter_index: usize,
    },
    HitInanimate {
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
    },
    HitPlayer {
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
        target_index: usize,
        target_health: u8,
    },
    Expire {
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientMessage {
    SendPasscode(Vec<u8>),
    SetUsername(String),
    SendChat(String),
    RequestStartGame,
    SetDifficulty(u8),
    EnterAfterGameChat,
    Input(WireItem<PlayerInput>),
}

pub fn version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("failed to get major version")
        .parse()
        .expect("failed to parse major version")
}
