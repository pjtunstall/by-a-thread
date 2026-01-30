use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use super::start_countdown::handle_countdown_started;
use crate::{
    assets::Assets,
    lobby::ui::{LobbyUi, UiErrorKind},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let Lobby::Chat {
        awaiting_initial_roster: _,
        waiting_for_server,
    } = lobby_state
    else {
        unreachable!();
    };

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        *waiting_for_server = false;

        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::CountdownStarted {
                    end_time,
                    game_data,
                },
                _,
            )) => {
                return Some(handle_countdown_started(end_time, game_data, assets));
            }
            Ok((ServerMessage::BeginDifficultySelection, _)) => {
                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false,
                }));
            }
            Ok((ServerMessage::DenyDifficultySelection, _)) => {
                *waiting_for_server = false;
            }
            Ok((
                ServerMessage::ChatMessage {
                    username,
                    color,
                    content,
                },
                _,
            )) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message_with_color(&format!("{}: {}", username, content), color);
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} left the chat.", username));
            }
            Ok((ServerMessage::Roster { online }, _)) => {
                if online.is_empty() {
                    ui.show_sanitized_message("Server: You are the only player online.");
                } else {
                    ui.show_sanitized_message("Server: Players online:");
                    for entry in online {
                        ui.show_sanitized_message_with_color(
                            &format!(" - {}", entry.username),
                            entry.color,
                        );
                    }
                }
                session.mark_initial_roster_received();
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((ServerMessage::AppointHost, _)) => {
                session.is_host = true;
                ui.show_sanitized_message(
                    "Server: You have been appointed host. Press TAB to begin.",
                );
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[DESERIALIZATION ERROR: {}]", e),
            ),
        }
    }

    while let Some(input) = session.take_input() {
        if input == "\t" {
            if session.is_host {
                let message = ClientMessage::RequestStartGame;
                let payload =
                    encode_to_vec(&message, standard()).expect("failed to serialize command");
                network.send_message(AppChannel::ReliableOrdered, payload);
                *waiting_for_server = true;
            }
            continue;
        }

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
            show_error: true,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_helpers::MockNetwork, test_helpers::MockUi};
    use common::chat::MAX_CHAT_MESSAGE_BYTES;

    #[test]
    fn enforces_max_message_length() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: true,
            waiting_for_server: false,
        }));
        session.mark_initial_roster_received();
        session.is_host = true;

        let long_message = "a".repeat(MAX_CHAT_MESSAGE_BYTES + 1);

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(
                    lobby_state,
                    &mut session,
                    &mut MockUi::new(),
                    &mut MockNetwork::new(),
                    None,
                )
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        session.add_input(long_message.clone());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert_eq!(network.sent_messages.len(), 1);

        let (_, payload) = network.sent_messages.pop_front().unwrap();
        let (message, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();

        match message {
            ClientMessage::SendChat(content) => {
                assert_eq!(content.len(), MAX_CHAT_MESSAGE_BYTES + 1);
            }
            _ => panic!("expected SendChat message"),
        }
    }

    #[test]
    fn sanitizes_chat_messages_ansi_and_control_chars() {
        let bell = "\x07";
        let red = "\x1B[31m";
        let reset = "\x1B[0m";

        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: true,
            waiting_for_server: false,
        }));
        session.mark_initial_roster_received();
        session.is_host = true;

        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        let malicious_chat = ServerMessage::ChatMessage {
            username: format!("Hacker{}", bell),
            color: common::player::Color::RED,
            content: format!("This is {}Danger{}!", red, reset),
        };

        network.queue_server_message(malicious_chat);

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Hacker: This is Danger!");
    }

    #[test]
    fn sends_start_game_request_on_tab_input() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: true,
            waiting_for_server: false,
        }));
        session.mark_initial_roster_received();
        session.is_host = true;

        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.add_input("\t".to_string());

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(_next_state.is_none());
        assert_eq!(network.sent_messages.len(), 1);
        let (channel, payload) = network.sent_messages.pop_front().unwrap();
        assert_eq!(channel, AppChannel::ReliableOrdered);

        let (message, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();
        assert!(matches!(message, ClientMessage::RequestStartGame));
        assert!(
            matches!(
                &session.state,
                ClientState::Lobby(Lobby::Chat {
                    waiting_for_server: true,
                    ..
                })
            ),
            "waiting_for_server should be true after sending request"
        );
    }
}
