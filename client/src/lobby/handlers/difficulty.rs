use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, InputMode, Lobby},
};
use shared::{
    input::UiKey,
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

const INVALID_CHOICE_MESSAGE: &str = "Invalid choice. Please press 1, 2, or 3.";

fn enqueue_difficulty_input(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> Option<ClientState> {
    if !matches!(session.input_mode(), InputMode::SingleKey) {
        return None;
    }

    match ui.poll_single_key() {
        Ok(Some(UiKey::Char(c))) if matches!(c, '1' | '2' | '3') => {
            session.add_input(c.to_string());
        }
        Err(e @ UiInputError::Disconnected) => {
            ui.show_sanitized_error(&format!("No connection: {}.", e));
            return Some(ClientState::TransitioningToDisconnected {
                message: e.to_string(),
            });
        }
        _ => {}
    }

    None
}

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let is_correct_state = matches!(
        session.state(),
        ClientState::Lobby(Lobby::ChoosingDifficulty { .. })
    );
    if !is_correct_state {
        panic!(
            "called difficulty::handle() when state was not ChoosingDifficulty; current state: {:?}",
            session.state()
        );
    };

    if let Some(next_state) = enqueue_difficulty_input(session, ui) {
        return Some(next_state);
    }

    if let ClientState::Lobby(Lobby::ChoosingDifficulty {
        prompt_printed,
        choice_sent,
    }) = session.state_mut()
    {
        if !*prompt_printed && !*choice_sent {
            ui.show_message("Server: Choose a difficulty level:");
            ui.show_message("  1. Easy");
            ui.show_message("  2. So-so");
            ui.show_message("  3. Next level");
            ui.show_prompt("Press 1, 2, or 3.");
            *prompt_printed = true;
        }
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
                return Some(ClientState::Lobby(Lobby::Countdown {
                    end_time,
                    maze,
                    players,
                }));
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                if let ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed,
                    choice_sent,
                }) = session.state_mut()
                {
                    *choice_sent = false;
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[DESERIALIZATION ERROR: {}]", e),
            ),
        }
    }

    let choice_already_sent = if let ClientState::Lobby(Lobby::ChoosingDifficulty {
        choice_sent,
        ..
    }) = session.state()
    {
        *choice_sent
    } else {
        false
    };

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

                if let ClientState::Lobby(Lobby::ChoosingDifficulty { choice_sent, .. }) =
                    session.state_mut()
                {
                    *choice_sent = true;
                }
            }
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
        return Some(ClientState::TransitioningToDisconnected {
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
    use shared::protocol::{ClientMessage, ServerMessage};

    #[test]
    #[should_panic(
        expected = "called difficulty::handle() when state was not ChoosingDifficulty; current state: Lobby(Startup { prompt_printed: false })"
    )]
    fn guards_panics_if_not_in_correct_state() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        handle(&mut session, &mut ui, &mut network);
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
            handle(&mut session, &mut ui, &mut network).is_none(),
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

        let next = handle(&mut session, &mut ui, &mut network);

        assert!(next.is_none());

        assert!(
            matches!(
                session.state(),
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

        let next = handle(&mut session, &mut ui, &mut network);

        assert!(next.is_none());

        assert!(
            matches!(
                session.state(),
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

        let next = handle(&mut session, &mut ui, &mut network);

        assert!(
            matches!(next, Some(ClientState::TransitioningToDisconnected { .. })),
            "expected transition to disconnected, got {:?}",
            next
        );
    }
}
