use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    assets::Assets,
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, InputMode},
};
use common::{
    chat::MAX_CHAT_MESSAGE_BYTES,
    constants::TICK_SECS,
    net::AppChannel,
    protocol::{AfterGameExitReason, ClientMessage, ServerMessage},
};

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::AfterGameChat { .. }) {
        panic!(
            "called after_game_chat::update() when state was not AfterGameChat; current state: {:?}",
            session.state()
        );
    }

    if matches!(session.input_mode(), InputMode::Enabled) {
        let ui_ref: &mut dyn LobbyUi = ui;
        match ui_ref.poll_input(MAX_CHAT_MESSAGE_BYTES, session.is_host) {
            Ok(Some(input)) => session.add_input(input),
            Err(UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", UiInputError::Disconnected));
                return Some(ClientState::Disconnected {
                    message: UiInputError::Disconnected.to_string(),
                });
            }
            Ok(None) => {}
        }
    }

    if let Some(next_state) = handle(session, ui, network) {
        return Some(next_state);
    }

    let ui_state = session.prepare_ui_state();
    if ui_state.show_waiting_message {
        ui.show_warning("Waiting for server...");
    }

    let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
    let show_cursor = should_show_input;
    let font = assets.map(|assets| &assets.font);
    ui.draw(should_show_input, show_cursor, font);

    None
}

fn handle(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        session.set_chat_waiting_for_server(false);

        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ChatMessage { username, content }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message(&format!("{}: {}", username, content));
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message(&format!("{} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message(&format!("{} left the chat.", username));
            }
            Ok((ServerMessage::AfterGameRoster { hades_shades }, _)) => {
                let message = if hades_shades.is_empty() {
                    "Server: You are the only shade in Hades.".to_string()
                } else {
                    format!("Shades in Hades: {}", hades_shades.join(", "))
                };
                ui.show_sanitized_message(&message);
                session.mark_initial_roster_received();
            }
            Ok((ServerMessage::AfterGameLeaderboard { entries }, _)) => {
                ui.show_sanitized_banner_message("Leaderboard:");
                for (rank, entry) in entries.iter().enumerate() {
                    let seconds = entry.ticks_survived as f64 * TICK_SECS;
                    let reason = match entry.exit_reason {
                        AfterGameExitReason::Disconnected => "disconnected",
                        AfterGameExitReason::Slain => "slain",
                        AfterGameExitReason::Winner => "winner",
                    };
                    ui.show_sanitized_banner_message(&format!(
                        "{}. {}  {:.1}s  ({})",
                        rank + 1,
                        entry.username,
                        seconds,
                        reason
                    ));
                }
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((_, _)) => {}
            Err(error) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[Deserialization error: {}]", error),
            ),
        }
    }

    while let Some(input) = session.take_input() {
        let trimmed_input = input.trim();

        if trimmed_input.is_empty() {
            continue;
        }

        let message = ClientMessage::SendChat(trimmed_input.to_string());

        let payload = encode_to_vec(&message, standard()).expect("failed to serialize chat");
        network.send_message(AppChannel::ReliableOrdered, payload);

        session.set_chat_waiting_for_server(true);
    }

    if network.is_disconnected() {
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!(
                "Disconnected from chat: {}.",
                network.get_disconnect_reason()
            ),
        );
        Some(ClientState::Disconnected {
            message: format!(
                "Disconnected from chat: {}.",
                network.get_disconnect_reason()
            ),
        })
    } else {
        None
    }
}
