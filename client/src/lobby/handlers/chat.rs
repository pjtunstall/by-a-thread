use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
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
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Lobby(Lobby::Chat { .. })) {
        panic!(
            "called chat::handle() when state was not Chat; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        session.set_chat_waiting_for_server(false);

        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::CountdownStarted { end_time, snapshot }, _)) => {
                return Some(ClientState::Lobby(Lobby::Countdown { end_time, snapshot }));
            }
            Ok((ServerMessage::BeginDifficultySelection, _)) => {
                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false,
                }));
            }
            // Client asked to choose difficulty, but was not the host,
            // so restore input prompt.
            // This shouldn't happen, but we have this mechanism as an
            // extra layer of protection.
            Ok((ServerMessage::DenyDifficultySelection, _)) => {
                session.set_chat_waiting_for_server(false);
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
                    "Server: You are the only player online.".to_string()
                } else {
                    format!("Players online: {}", online.join(", "))
                };
                ui.show_sanitized_message(&msg);
                session.mark_initial_roster_received();
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((ServerMessage::AppointHost, _)) => {
                session.is_host = true;
                ui.show_sanitized_message("You have been appointed host. Press TAB to begin.");
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[Deserialization error: {}]", e),
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
                session.set_chat_waiting_for_server(true);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_helpers::MockNetwork, test_helpers::MockUi};
    use common::chat::MAX_CHAT_MESSAGE_BYTES;

    mod guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called chat::handle() when state was not Chat; current state: Lobby(Startup { prompt_printed: false })"
        )]
        fn chat_panics_if_not_in_chat_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn chat_does_not_panic_in_chat_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Lobby(Lobby::Chat {
                awaiting_initial_roster: true,
                waiting_for_server: false,
            }));
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
        session.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: true,
            waiting_for_server: false,
        }));
        session.mark_initial_roster_received();
        session.is_host = true;

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
            content: format!("This is {}Danger{}!", red, reset),
        };

        network.queue_server_message(malicious_chat);

        handle(&mut session, &mut ui, &mut network);

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

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert_eq!(network.sent_messages.len(), 1);
        let (channel, payload) = network.sent_messages.pop_front().unwrap();
        assert_eq!(channel, AppChannel::ReliableOrdered);

        let (message, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();
        assert!(matches!(message, ClientMessage::RequestStartGame));
        assert!(session.chat_waiting_for_server());
    }
}
