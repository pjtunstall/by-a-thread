use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{net::NetworkHandle, session::ClientSession, state::ClientState, ui::ClientUi};
use shared::{
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let is_correct_state = matches!(session.state(), ClientState::ChoosingDifficulty { .. });
    if !is_correct_state {
        panic!(
            "called difficulty::handle() when state was not ChoosingDifficulty; current state: {:?}",
            session.state()
        );
    };

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
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                if let ClientState::ChoosingDifficulty {
                    prompt_printed,
                    choice_sent,
                } = session.state_mut()
                {
                    *choice_sent = false;
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    let choice_already_sent =
        if let ClientState::ChoosingDifficulty { choice_sent, .. } = session.state() {
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
                    ui.show_sanitized_error("Invalid choice. Please press 1, 2, or 3.");
                    None
                }
            };

            if let Some(level) = level {
                let msg = ClientMessage::SetDifficulty(level);
                let payload =
                    encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
                network.send_message(AppChannel::ReliableOrdered, payload);

                if let ClientState::ChoosingDifficulty { choice_sent, .. } = session.state_mut() {
                    *choice_sent = true;
                }
            }
        }
    } else {
        session.take_input();
    }

    if network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "Disconnected while choosing difficulty: {}.",
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
    use shared::protocol::ServerMessage;

    #[test]
    #[should_panic(
        expected = "called difficulty::handle() when state was not ChoosingDifficulty; current state: Startup"
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
        session.transition(ClientState::ChoosingDifficulty {
            prompt_printed: false,
            choice_sent: false,
        });
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
        session.transition(ClientState::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: true,
        });

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: "Pick again".to_string(),
        });

        let next = handle(&mut session, &mut ui, &mut network);

        assert!(next.is_none());
        if let ClientState::ChoosingDifficulty {
            prompt_printed,
            choice_sent,
        } = session.state()
        {
            assert!(!*choice_sent, "choice_sent should reset on server info");
            assert!(
                !*prompt_printed,
                "prompt_printed should reset so prompt can be shown again"
            );
        } else {
            panic!("state unexpectedly changed");
        }
        assert_eq!(
            ui.messages.len(),
            1,
            "server info should be surfaced to the user"
        );
    }
}
