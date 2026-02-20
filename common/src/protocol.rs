use glam::Vec3;
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::{
    player::{Color, PlayerInput},
    ring::WireItem,
    snapshot::{InitialData, Snapshot},
};

pub const MAX_CLIENT_MESSAGE_BYTES: usize = 512;

pub const GAME_ALREADY_STARTED_MESSAGE: &str =
    "The game is already in progress. Please try again after this match.";

pub const AUTH_SUCCESS_MESSAGE: &str = "You're in.";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Display)]
#[strum(serialize_all = "lowercase")]
pub enum PostGameExitReason {
    Disconnected,
    Shot,
    Winner,
    Minotaured,
    Escaped,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostGameLeaderboardEntry {
    pub username: String,
    pub color: Color,
    pub ticks_survived: u64,
    pub exit_reason: PostGameExitReason,
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
    PostGameRoster {
        hades_shades: Vec<PlayerRosterEntry>,
    },
    PostGameLeaderboard {
        entries: Vec<PostGameLeaderboardEntry>,
    },
    ServerInfo {
        message: String,
    },
    LobbyTimer {
        end_time: f64,
    },
    BeginDifficultySelection, // Allow host to move to phase where they choose a difficulty.
    DenyDifficultySelection,  // Refuse non-host client who asks to choose a difficulty level.
    Victory {
        winner_index: usize,
    },
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
            Self::PostGameRoster { .. } => "PostGameRoster",
            Self::PostGameLeaderboard { .. } => "PostGameLeaderboard",
            Self::ServerInfo { .. } => "ServerInfo",
            Self::LobbyTimer { .. } => "LobbyTimer",
            Self::BeginDifficultySelection => "BeginDifficultySelection",
            Self::DenyDifficultySelection => "DenyDifficultySelection",
            Self::Victory { .. } => "Victory",
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
    SetUsername(String),
    SendChat(String),
    RequestStartGame,
    SetDifficulty(u8),
    EnterPostGameChat,
    Input(WireItem<PlayerInput>),
}

use std::sync::OnceLock;

static VERSION_STRING: OnceLock<&'static str> = OnceLock::new();

pub fn version_string() -> &'static str {
    *VERSION_STRING.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        let commit = env!("BUILD_GIT_COMMIT");
        if commit.is_empty() {
            version
        } else {
            const MAX_DISPLAY: usize = 12;
            let short = if commit.len() > MAX_DISPLAY {
                &commit[..MAX_DISPLAY]
            } else {
                commit
            };
            Box::leak(format!("{}-{}", version, short).into_boxed_str())
        }
    })
}

pub fn protocol_id() -> u64 {
    let commit = env!("BUILD_GIT_COMMIT");
    if commit.is_empty() {
        let v = env!("CARGO_PKG_VERSION");
        let parts: Vec<u64> = v.split('.').map(|s| s.parse().unwrap_or(0)).collect();
        let (major, minor, patch) = (
            parts.get(0).copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        );
        major * 1_000_000 + minor * 1_000 + patch
    } else {
        const LEN: usize = 16;
        if commit.len() >= LEN {
            u64::from_str_radix(&commit[..LEN], 16).unwrap_or_else(|_| hash_commit(commit))
        } else {
            hash_commit(commit)
        }
    }
}

fn hash_commit(commit: &str) -> u64 {
    let mut hash: u64 = 0;
    for (i, b) in commit.bytes().take(16).enumerate() {
        hash ^= (b as u64) << (i % 8 * 8);
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}
