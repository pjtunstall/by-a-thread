use macroquad::prelude::*;

use crate::{
    lobby::handlers,
    net::RenetNetworkHandle,
    session::ClientSession,
    state::{ClientState, LobbyState},
};

pub enum LobbyStep {
    Continue,
    StartGame,
    Transition(ClientState),
}

pub async fn lobby_frame(
    session: &mut ClientSession,
    ui: &mut dyn crate::lobby::ui::LobbyUi,
    network_handle: &mut RenetNetworkHandle<'_>,
    is_host: bool,
) -> LobbyStep {
    if session.state().is_disconnected() {
        handle_disconnected_ui_loop(ui).await;
        return LobbyStep::Continue;
    }

    if matches!(session.input_mode(), crate::state::InputMode::Enabled) {
        let ui_ref: &mut dyn crate::lobby::ui::LobbyUi = ui;
        match ui_ref.poll_input(shared::chat::MAX_CHAT_MESSAGE_BYTES, is_host) {
            Ok(Some(input)) => session.add_input(input),
            Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", e));
                return LobbyStep::Transition(ClientState::Disconnected {
                    message: e.to_string(),
                });
            }
            Ok(None) => {}
        }
    }

    if session.is_countdown_finished() {
        return LobbyStep::StartGame;
    }

    if let Some(next_state) = update_lobby_state(session, ui, network_handle) {
        return LobbyStep::Transition(next_state);
    }

    let ui_state = session.prepare_ui_state();
    if ui_state.show_waiting_message {
        ui.show_warning("Waiting for server...");
    }

    if !session.is_countdown_active() {
        let should_show_input = matches!(ui_state.mode, crate::state::InputMode::Enabled);
        let show_cursor = should_show_input;
        ui.draw(should_show_input, show_cursor);
    }

    LobbyStep::Continue
}

fn update_lobby_state(
    session: &mut ClientSession,
    ui: &mut dyn crate::lobby::ui::LobbyUi,
    network_handle: &mut RenetNetworkHandle<'_>,
) -> Option<ClientState> {
    match session.state() {
        ClientState::Lobby(LobbyState::Startup { .. }) => handlers::startup::handle(session, ui),
        ClientState::Lobby(LobbyState::Connecting { .. }) => {
            handlers::connecting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::Authenticating { .. }) => {
            handlers::auth::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::ChoosingUsername { .. }) => {
            handlers::username::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::AwaitingUsernameConfirmation) => {
            handlers::waiting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::InChat { .. }) => {
            handlers::chat::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::ChoosingDifficulty { .. }) => {
            handlers::difficulty::handle(session, ui, network_handle)
        }
        ClientState::Lobby(LobbyState::Countdown { .. }) => {
            handlers::countdown::handle(session, ui, network_handle)
        }
        ClientState::Disconnected { .. } => None,
        ClientState::Game(_) => None,
        ClientState::Debrief => None,
    }
}

async fn handle_disconnected_ui_loop(ui: &mut dyn crate::lobby::ui::LobbyUi) {
    loop {
        ui.draw(false, false);
        if is_key_pressed(KeyCode::Escape) || is_quit_requested() {
            break;
        }

        next_frame().await;
    }
}
