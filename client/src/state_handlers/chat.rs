use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState};
use crate::{
    net::NetworkHandle,
    ui::{ClientUi, UiInputError},
};
use shared::{
    net::AppChannel,
    {
        chat::MAX_CHAT_MESSAGE_BYTES,
        protocol::{ClientMessage, ServerMessage},
    },
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::InChat) {
        panic!(
            "called in_chat() when state was not InChat; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::CountdownStarted {
                    end_time,
                    maze,
                    players,
                },
                _,
            )) => {
                session.countdown_end_time = Some(end_time);
                session.maze = Some(maze);
                session.players = Some(players);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::RequestDifficultyChoice, _)) => {
                return Some(ClientState::ChoosingDifficulty {
                    prompt_printed: false,
                });
            }
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
            Ok((ServerMessage::Roster { online }, _)) => {
                let msg = if online.is_empty() {
                    "You are the only player online.".to_string()
                } else {
                    format!("Players online: {}", online.join(", "))
                };
                ui.show_sanitized_message(&msg);
                session.mark_initial_roster_received();
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    loop {
        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
            Ok(Some(input)) => {
                let trimmed_input = input.trim();
                if !trimmed_input.is_empty() {
                    let message = if trimmed_input == shared::auth::START_COUNTDOWN {
                        ClientMessage::RequestStartGame
                    } else {
                        ClientMessage::SendChat(trimmed_input.to_string())
                    };

                    let payload =
                        encode_to_vec(&message, standard()).expect("failed to serialize chat");
                    network.send_message(AppChannel::ReliableOrdered, payload);
                }
            }
            Ok(None) => break,
            Err(UiInputError::Disconnected) => {
                return Some(ClientState::Disconnected {
                    message: "input thread disconnected.".to_string(),
                });
            }
        }
    }

    if network.is_disconnected() {
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
