mod flow;
pub mod state;
mod state_handlers;

use std::net::SocketAddr;

use macroquad::prelude::*;
use renet_netcode::ConnectToken;

use crate::{
    assets::Assets,
    exit,
    lobby::ui::{Gui, LobbyUi},
    session::ClientSession,
};

pub use state::{ApiRequestPhase, MatchmakerResponse, PreLobby};

pub struct PreLobbyResult {
    pub session: ClientSession,
    pub ui: Gui,
    pub assets: Assets,
    pub connect_token: ConnectToken,
    pub server_addr: SocketAddr,
    pub share_passcode: Option<String>,
    pub only_player: bool,
}

pub async fn run_pre_lobby_loop(
    session: ClientSession,
    ui: Gui,
    assets: Assets,
) -> Option<PreLobbyResult> {
    let mut session = session;
    let mut ui = ui;

    loop {
        match flow::update(&mut session, &mut ui, Some(&assets)) {
            flow::PreLobbyStep::Continue => {}
            flow::PreLobbyStep::Complete(info) => {
                return Some(PreLobbyResult {
                    session,
                    ui,
                    assets,
                    connect_token: info.connect_token,
                    server_addr: info.server_addr,
                    share_passcode: info.share_passcode,
                    only_player: info.only_player,
                });
            }
            flow::PreLobbyStep::Exit => return None,
            flow::PreLobbyStep::ExitPendingUserAck => {
                ui.show_message(" ");
                exit::wait_till_escape_is_pressed(&mut ui, Some(&assets.font)).await;
                return None;
            }
        }
        next_frame().await;
    }
}
