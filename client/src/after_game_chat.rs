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
    protocol::{ClientMessage, ServerMessage},
};

#[derive(Debug)]
pub struct AfterGameChat {
    pub awaiting_initial_roster: bool,
    pub waiting_for_server: bool,
}

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let state = std::mem::take(&mut session.state);

    let result = match state {
        ClientState::AfterGameChat(mut chat_state) => {
            let result = handle(&mut chat_state, session, ui, network);
            session.state = ClientState::AfterGameChat(chat_state);
            result
        }
        other_state => {
            session.state = other_state;
            None
        }
    };

    if result.is_some() {
        return result;
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
    chat_state: &mut AfterGameChat,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let AfterGameChat {
        awaiting_initial_roster,
        waiting_for_server,
    } = chat_state;

    if matches!(session.input_mode(), InputMode::Enabled) {
        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES, session.is_host) {
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

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        *waiting_for_server = false;

        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::ChatMessage {
                    username,
                    color,
                    content,
                },
                _,
            )) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message_with_color(&format!("{}: {}", username, content), color);
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} left the chat.", username));
            }
            Ok((ServerMessage::AfterGameRoster { hades_shades }, _)) => {
                if hades_shades.is_empty() {
                    ui.show_sanitized_message("Server: You are the only shade in Hades.");
                } else {
                    ui.show_sanitized_message("Server: Shades in Hades:");
                    for entry in hades_shades {
                        ui.show_sanitized_message_with_color(
                            &format!(" - {}", entry.username),
                            entry.color,
                        );
                    }
                }
                *awaiting_initial_roster = false;
            }
            Ok((ServerMessage::AfterGameLeaderboard { entries }, _)) => {
                ui.show_sanitized_message("Leaderboard:");
                let mut current_rank = 1;
                let mut prev_ticks: Option<u64> = None;
                for entry in entries.iter() {
                    if prev_ticks.is_some() && prev_ticks != Some(entry.ticks_survived) {
                        current_rank += 1;
                    }
                    prev_ticks = Some(entry.ticks_survived);

                    let seconds = entry.ticks_survived as f64 * TICK_SECS;
                    ui.show_sanitized_message_with_color(
                        &format!(
                            "  {}. {}  {:.1}s  ({})",
                            current_rank, entry.username, seconds, entry.exit_reason
                        ),
                        entry.color,
                    );
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

        *waiting_for_server = true;
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
