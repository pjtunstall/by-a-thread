use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState};
use crate::{net::NetworkHandle, ui::ClientUi};
use shared::{
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::InChat) {
        panic!(
            "called chat::handle() when state was not InChat; current state: {:?}",
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

    while let Some(input) = session.take_input() {
        let trimmed_input = input.trim();
        if !trimmed_input.is_empty() {
            let message = if trimmed_input == shared::auth::START_COUNTDOWN {
                ClientMessage::RequestStartGame
            } else {
                ClientMessage::SendChat(trimmed_input.to_string())
            };

            let payload = encode_to_vec(&message, standard()).expect("failed to serialize chat");
            network.send_message(AppChannel::ReliableOrdered, payload);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::{ClientSession, ClientState},
        test_helpers::MockNetwork,
        test_helpers::MockUi,
    };
    use shared::chat::MAX_CHAT_MESSAGE_BYTES;

    mod guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called chat::handle() when state was not InChat; current state: Startup"
        )]
        fn in_chat_panics_if_not_in_in_chat_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn in_chat_does_not_panic_in_in_chat_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::InChat);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn enforces_max_message_length() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::InChat);
        session.mark_initial_roster_received();

        let long_message = "a".repeat(MAX_CHAT_MESSAGE_BYTES + 1);

        session.add_input(long_message.clone());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(network.sent_messages.len(), 1);

        let (_, payload) = network.sent_messages.pop_front().unwrap();
        let (message, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();

        match message {
            ClientMessage::SendChat(content) => {
                assert_eq!(content.len(), MAX_CHAT_MESSAGE_BYTES + 1);
            }
            _ => panic!("Expected SendChat message"),
        }
    }

    #[test]
    fn sanitizes_chat_messages_ansi_and_control_chars() {
        let bell = "\x07";
        let red = "\x1B[31m";
        let reset = "\x1B[0m";

        let mut session = ClientSession::new(0);
        session.transition(ClientState::InChat);
        session.mark_initial_roster_received();

        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        let malicious_chat = ServerMessage::ChatMessage {
            username: format!("Hacker{}", bell),
            content: format!("This is {}Danger{}!", red, reset),
        };

        network.queue_server_message(malicious_chat);

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Hacker: This is Danger!");
    }
}
