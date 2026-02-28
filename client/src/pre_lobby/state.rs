use std::{net::SocketAddr, sync::mpsc};

use renet_netcode::ConnectToken;

use crate::matchmaker::{MatchmakerError, CreateGameResponse, JoinGameResponse};
use common::auth::Passcode;

#[derive(Debug)]
pub enum PreLobby {
    ServerAddress { prompt_printed: bool },
    MatchmakerRequestMenu {
        matchmaker_host: String,
        phase: MatchmakerRequestPhase,
    },
}

#[derive(Debug)]
pub enum MatchmakerRequestPhase {
    ChoosingNewOrJoin {
        selected_index: usize,
        prompt_printed: bool,
    },
    AwaitingPing {
        matchmaker_host: String,
        receiver: mpsc::Receiver<Result<(), String>>,
    },
    ChoosingPlayerCount { prompt_printed: bool },
    ChoosingPasscode {
        wrong_guesses: u8,
        prompt_printed: bool,
    },
    AwaitingCreate {
        player_count: u8,
        receiver: mpsc::Receiver<Result<(CreateGameResponse, SocketAddr), MatchmakerError>>,
    },
    AwaitingJoin {
        passcode: String,
        wrong_guesses: u8,
        receiver: mpsc::Receiver<Result<(JoinGameResponse, SocketAddr), MatchmakerError>>,
    },
}

pub enum MatchmakerResponse {
    Create {
        server_address: SocketAddr,
        connect_token: ConnectToken,
        passcode: Passcode,
        player_count: u8,
    },
    Join {
        server_address: SocketAddr,
        connect_token: ConnectToken,
        passcode: Passcode,
    },
}

impl MatchmakerResponse {
    pub fn only_player(&self) -> bool {
        matches!(self, Self::Create { player_count, .. } if *player_count == 1)
    }

    pub fn server_address(&self) -> SocketAddr {
        match self {
            Self::Create { server_address, .. } | Self::Join { server_address, .. } => *server_address,
        }
    }

    pub fn connect_token(self) -> ConnectToken {
        match self {
            Self::Create { connect_token, .. } | Self::Join { connect_token, .. } => connect_token,
        }
    }

    pub fn passcode(&self) -> &Passcode {
        match self {
            Self::Create { passcode, .. } | Self::Join { passcode, .. } => passcode,
        }
    }

    pub fn is_host(&self) -> bool {
        matches!(self, Self::Create { .. })
    }

    pub fn share_passcode(&self) -> Option<String> {
        match self {
            Self::Create { passcode, .. } => Some(passcode.string.clone()),
            Self::Join { .. } => None,
        }
    }
}
