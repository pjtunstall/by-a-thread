use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use super::start_countdown::handle_countdown_started;
use crate::{
    assets::Assets,
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    input::UiKey,
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

const INVALID_CHOICE_MESSAGE: &str = "Invalid choice. Please press 1, 2, or 3.";

fn enqueue_difficulty_input(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    choice_sent: bool,
) -> Option<ClientState> {
    // Don't use session.input_mode() here because session.state has been extracted
    // by std::mem::take in flow.rs. Instead, determine input mode from choice_sent.
    if choice_sent {
        // Already sent, don't accept more input
        return None;
    }

    match ui.poll_single_key() {
        Ok(key_result) => match key_result {
            Some(UiKey::Char(c)) if matches!(c, '1' | '2' | '3') => {
                session.add_input(c.to_string());
            }
            _ => {}
        },
        Err(UiInputError::Disconnected) => {
            ui.show_sanitized_error("No connection: disconnected.");
            return Some(ClientState::Disconnected {
                message: "disconnected".to_string(),
            });
        }
    }

    None
}

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let Lobby::ChoosingDifficulty {
        prompt_printed,
        choice_sent,
    } = lobby_state
    else {
        unreachable!();
    };

    if let Some(next_state) = enqueue_difficulty_input(session, ui, *choice_sent) {
        return Some(next_state);
    }

    if !*prompt_printed && !*choice_sent {
        ui.show_message("Server: Choose a difficulty level:");
        ui.show_message("  1. Easy");
        ui.show_message("  2. So-so");
        ui.show_message("  3. Next level");
        ui.show_prompt("Press 1, 2, or 3.");
        *prompt_printed = true;
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
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
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                // Reset state when server info received
                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false,
                }));
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[DESERIALIZATION ERROR: {}]", e),
            ),
        }
    }

    // Check if choice was already sent via lobby_state
    let choice_already_sent = *choice_sent;

    if !choice_already_sent {
        if let Some(input) = session.take_input() {
            let trimmed = input.trim();
            let level = match trimmed {
                "1" => Some(1),
                "2" => Some(2),
                "3" => Some(3),
                _ => {
                    ui.show_typed_error(
                        UiErrorKind::DifficultyInvalidChoice,
                        INVALID_CHOICE_MESSAGE,
                    );
                    None
                }
            };

            if let Some(level) = level {
                let msg = ClientMessage::SetDifficulty(level);
                let payload =
                    encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
                network.send_message(AppChannel::ReliableOrdered, payload);

                // Return updated state with choice_sent set to true
                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: *prompt_printed,
                    choice_sent: true,
                }));
            }
        } else {
            session.take_input();
        }
    } else {
        session.take_input();
    }

    if network.is_disconnected() {
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!(
                "disconnected while choosing difficulty: {}",
                network.get_disconnect_reason()
            ),
        );
        return Some(ClientState::Disconnected {
            message: format!(
                "disconnected while choosing difficulty: {}",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_helpers::MockNetwork, test_helpers::MockUi};
    use common::protocol::{ClientMessage, ServerMessage};

    #[test]
    #[should_panic(
        expected = "called difficulty::handle() with non-ChoosingDifficulty state: Lobby(ServerAddress { prompt_printed: false })"
    )]
    fn guards_panics_if_not_in_correct_state() {
        let mut session = ClientSession::new(0);
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
    }

    #[test]
    fn guards_does_not_panic_in_correct_state() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: false,
            choice_sent: false,
        }));
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        assert!(
            {
                let mut temp_state = std::mem::take(&mut session.state);
                let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                    handle(lobby_state, &mut session, &mut ui, &mut network, None)
                } else {
                    panic!("expected Lobby state");
                };
                session.state = temp_state;
                result
            }
            .is_none(),
            "should not panic and should return None"
        );
    }

    #[test]
    fn re_enables_input_after_server_info() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: true,
        }));

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: INVALID_CHOICE_MESSAGE.to_string(),
        });

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

        assert!(
            matches!(
                &session.state,
                ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false
                })
            ),
            "state should reset prompt_printed and choice_sent to false"
        );

        assert_eq!(
            ui.messages.len(),
            1,
            "server info should be surfaced to the user"
        );
    }

    #[test]
    fn polls_single_key_for_choice() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: false,
        }));

        let mut ui = MockUi::default();
        ui.keys.push_back(Ok(Some(UiKey::Char('2'))));
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

        assert!(_next_state.is_none());

        assert!(
            matches!(
                &session.state,
                ClientState::Lobby(Lobby::ChoosingDifficulty {
                    choice_sent: true,
                    ..
                })
            ),
            "choice should be marked as sent after pressing a key"
        );

        let (channel, payload) = network
            .sent_messages
            .pop_front()
            .expect("expected difficulty choice to be sent");
        assert_eq!(channel, AppChannel::ReliableOrdered);
        let (msg, _) =
            decode_from_slice::<ClientMessage, _>(&payload, standard()).expect("decode message");
        assert_eq!(msg, ClientMessage::SetDifficulty(2));
    }

    #[test]
    fn returns_disconnect_on_input_source_drop() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: false,
        }));

        let mut ui = MockUi::default();
        ui.keys.push_back(Err(UiInputError::Disconnected));
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

        assert!(
            matches!(_next_state, Some(ClientState::Disconnected { .. })),
            "expected transition to disconnected, got {:?}",
            _next_state
        );
    }
}
