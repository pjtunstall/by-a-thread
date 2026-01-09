use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    net::ServerNetworkHandle,
    state::{
        AuthAttemptOutcome, ChoosingDifficulty, Lobby, ServerState, evaluate_passcode_attempt,
    },
};
use common::{
    self,
    auth::{MAX_ATTEMPTS, Passcode},
    chat::MAX_CHAT_MESSAGE_BYTES,
    net::AppChannel,
    player::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username},
    protocol::{
        auth_success_message, ClientMessage, ServerMessage,
        AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE,
        AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyErrorKind {
    UsernameValidation(UsernameError),
}

pub fn handle(
    network: &mut dyn ServerNetworkHandle,
    state: &mut Lobby,
    passcode: &Passcode,
) -> Option<ServerState> {
    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!(
                    "client {} sent malformed data; disconnecting them",
                    client_id
                );
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::SendPasscode(guess_bytes) => {
                    if !state.is_authenticating(client_id) {
                        eprintln!("client {} sent passcode in wrong state", client_id);
                        continue;
                    }

                    let (outcome, attempts_count) = {
                        let attempts_entry = state
                            .authentication_attempts(client_id)
                            .expect("expected authentication state for client");
                        let outcome = evaluate_passcode_attempt(
                            passcode.bytes.as_slice(),
                            attempts_entry,
                            &guess_bytes,
                            MAX_ATTEMPTS,
                        );
                        let count = *attempts_entry;
                        (outcome, count)
                    };

                    match outcome {
                        AuthAttemptOutcome::Authenticated => {
                            println!("Client {} authenticated successfully.", client_id);
                            state.mark_authenticated(client_id);

                            let prompt = auth_success_message(MAX_USERNAME_LENGTH);
                            let message = ServerMessage::ServerInfo { message: prompt };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize ServerInfo");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                        }
                        AuthAttemptOutcome::TryAgain => {
                            println!(
                                "Client {} sent wrong passcode (Attempt {}).",
                                client_id, attempts_count
                            );

                            let message = ServerMessage::ServerInfo {
                                message: AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE.to_string(),
                            };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize ServerInfo");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                        }
                        AuthAttemptOutcome::Disconnect => {
                            eprintln!(
                                "client {} failed authentication; disconnecting them",
                                client_id
                            );
                            let message = ServerMessage::ServerInfo {
                                message: AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE.to_string(),
                            };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize ServerInfo");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                            network.disconnect(client_id);
                            state.remove_client(client_id, network);
                        }
                    }
                }
                ClientMessage::SetUsername(username_text) => {
                    if !state.needs_username(client_id) {
                        eprintln!("client {} sent username in wrong state", client_id);
                        continue;
                    }

                    match sanitize_username(&username_text) {
                        Ok(username) => {
                            if state.is_username_taken(&username) {
                                send_username_error(
                                    network,
                                    client_id,
                                    "username is already taken",
                                    LobbyErrorKind::UsernameValidation(UsernameError::Reserved),
                                );
                                continue;
                            }

                            state.register_username(client_id, &username);
                            println!("Client {} set username to '{}'.", client_id, username);

                            let message = ServerMessage::Welcome {
                                username: username.to_string(),
                            };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize Welcome");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);

                            let others = state.usernames_except(client_id);
                            let message = ServerMessage::Roster { online: others };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize Roster");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);

                            if state.usernames_except(client_id).is_empty() {
                                state.set_host(client_id, network);
                            }

                            let message = ServerMessage::UserJoined {
                                username: username.to_string(),
                            };
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize UserJoined");
                            network.broadcast_message_except(
                                client_id,
                                AppChannel::ReliableOrdered,
                                payload,
                            );
                        }
                        Err(err) => {
                            let error_text = match err {
                                UsernameError::Empty => "username must not be empty",
                                UsernameError::TooLong => "username is too long",
                                UsernameError::InvalidCharacter(_) => {
                                    "username contains invalid characters"
                                }
                                UsernameError::Reserved => "that username is reserved",
                            };
                            send_username_error(
                                network,
                                client_id,
                                error_text,
                                LobbyErrorKind::UsernameValidation(err),
                            );
                        }
                    }
                }
                ClientMessage::SendChat(content) => {
                    if let Some(username) = state.username(client_id) {
                        let clean_content = common::input::sanitize(&content);
                        let trimmed_content = clean_content.trim();

                        if trimmed_content.is_empty() {
                            continue;
                        }
                        if trimmed_content.len() > MAX_CHAT_MESSAGE_BYTES {
                            println!(
                                "Client {} sent an overly long chat message; ignoring.",
                                client_id
                            );
                            continue;
                        }

                        println!("{}: {}", username, trimmed_content);
                        let message = ServerMessage::ChatMessage {
                            username: username.to_string(),
                            content: trimmed_content.to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize ChatMessage");
                        network.broadcast_message(AppChannel::ReliableOrdered, payload);
                    } else {
                        eprintln!("client {} sent chat message in wrong state", client_id);
                    }
                }
                ClientMessage::RequestStartGame => {
                    if state.is_host(client_id) {
                        return Some(ServerState::ChoosingDifficulty(ChoosingDifficulty::new(
                            state,
                        )));
                    } else {
                        // Send refusal message to client so they can exit "Waiting for server..."
                        // 'state' and restore their input prompt. The client code should
                        // prevent a non-host client from asking to move to the difficulty
                        // choice state. This guarantees it.
                        eprintln!("client {} (not host) tried to start game", client_id);
                        let message = ServerMessage::DenyDifficultySelection;
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize NoHost message");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                    }
                }
                ClientMessage::SetDifficulty(_) => {
                    eprintln!(
                        "client {} sent SetDifficulty in lobby state; ignoring",
                        client_id
                    );
                }
                ClientMessage::Input(_) => {
                    eprintln!("client {} sent game input; ignoring", client_id)
                }
            }
        }
    }

    None
}

fn send_username_error(
    network: &mut dyn ServerNetworkHandle,
    client_id: u64,
    message: &str,
    _kind: LobbyErrorKind,
) {
    let message = ServerMessage::UsernameError {
        message: message.to_string(),
    };
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize UsernameError");
    network.send_message(client_id, AppChannel::ReliableOrdered, payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Lobby;
    use crate::test_helpers::MockServerNetwork;
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;
    use bincode::serde::encode_to_vec;
    use common::{
        auth::{MAX_ATTEMPTS, Passcode},
        protocol::{
            auth_success_message, ClientMessage, ServerMessage,
            AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE,
        },
    };

    #[test]
    fn auth_success() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);

        let msg = ClientMessage::SendPasscode(vec![1, 2, 3, 4, 5, 6]);
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert!(lobby_state.needs_username(1));
        assert!(next_state.is_none());

        let client_msgs = network.get_sent_messages_data(1);
        assert_eq!(client_msgs.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&client_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert_eq!(message, auth_success_message(MAX_USERNAME_LENGTH));
        } else {
            panic!("expected ServerInfo message, got {:?}", msg);
        }
    }

    #[test]
    fn auth_fail_then_disconnect() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);

        for _ in 0..MAX_ATTEMPTS {
            let msg = ClientMessage::SendPasscode(vec![0, 0, 0, 0, 0, 0]);
            let payload = encode_to_vec(&msg, standard()).unwrap();
            network.queue_raw_message(1, payload);
        }

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert_eq!(lobby_state.username(1), None);
        assert!(next_state.is_none());

        assert!(network.disconnected_clients.contains(&1));
        let client_msgs = network.get_sent_messages_data(1);
        let last_msg_data = client_msgs.last().unwrap();
        let msg = decode_from_slice::<ServerMessage, _>(last_msg_data, standard())
            .unwrap()
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert_eq!(message, AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE);
        } else {
            panic!("expected ServerInfo message, got {:?}", msg);
        }
    }

    #[test]
    fn username_success_and_broadcast() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);
        lobby_state.mark_authenticated(1);
        lobby_state.register_username(1, "Alice");

        network.add_client(2);
        lobby_state.register_connection(2);
        lobby_state.mark_authenticated(2);

        let msg = ClientMessage::SetUsername("Bob".to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(2, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert_eq!(lobby_state.username(2), Some("Bob"));
        assert!(next_state.is_none());

        let bob_msgs = network.get_sent_messages_data(2);
        assert_eq!(bob_msgs.len(), 2);

        let msg1 = decode_from_slice::<ServerMessage, _>(&bob_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::Welcome { username } = msg1 {
            assert_eq!(username, "Bob");
        } else {
            panic!("expected Welcome message, got {:?}", msg1);
        }

        let msg2 = decode_from_slice::<ServerMessage, _>(&bob_msgs[1], standard())
            .unwrap()
            .0;
        if let ServerMessage::Roster { online } = msg2 {
            assert_eq!(online, vec!["Alice"]);
        } else {
            panic!("expected Roster message, got {:?}", msg2);
        }

        let alice_msgs = network.get_sent_messages_data(1);
        assert_eq!(alice_msgs.len(), 1);
        let msg_alice = decode_from_slice::<ServerMessage, _>(&alice_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::UserJoined { username } = msg_alice {
            assert_eq!(username, "Bob");
        } else {
            panic!("expected UserJoined message, got {:?}", msg_alice);
        }
    }

    #[test]
    fn chat_message() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);
        lobby_state.mark_authenticated(1);
        lobby_state.register_username(1, "Alice");

        network.add_client(2);
        lobby_state.register_connection(2);
        lobby_state.mark_authenticated(2);
        lobby_state.register_username(2, "Bob");

        let msg = ClientMessage::SendChat("Hello Bob!".to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::ChatMessage { username, content } = msg {
            assert_eq!(username, "Alice");
            assert_eq!(content, "Hello Bob!");
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }

    #[test]
    fn chat_length_limit() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);
        lobby_state.mark_authenticated(1);
        lobby_state.register_username(1, "Alice");

        let long_message = "a".repeat(MAX_CHAT_MESSAGE_BYTES + 1);
        let msg = ClientMessage::SendChat(long_message);
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(
            broadcasts.len(),
            0,
            "server broadcasted a message that exceeded the length limit"
        );
    }

    #[test]
    fn chat_sanitization() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);
        lobby_state.mark_authenticated(1);
        lobby_state.register_username(1, "Alice");

        network.add_client(2);
        lobby_state.register_connection(2);
        lobby_state.mark_authenticated(2);
        lobby_state.register_username(2, "Bob");

        let malicious_content = "  Hello\x07Bob!\x1B[2J  ";
        let expected_content = "HelloBob!";

        let msg = ClientMessage::SendChat(malicious_content.to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard())
            .unwrap()
            .0;

        if let ServerMessage::ChatMessage { username, content } = msg {
            assert_eq!(username, "Alice");
            assert_eq!(
                content, expected_content,
                "chat content was not properly sanitized"
            );
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }

    #[test]
    fn reserved_username_is_rejected() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);
        lobby_state.mark_authenticated(1);

        let msg = ClientMessage::SetUsername("sErVeR".to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle(&mut network, &mut lobby_state, &passcode);

        assert!(next_state.is_none());
        assert_eq!(lobby_state.username(1), None);

        let client_msgs = network.get_sent_messages_data(1);
        assert_eq!(client_msgs.len(), 1);

        let msg = decode_from_slice::<ServerMessage, _>(&client_msgs[0], standard())
            .unwrap()
            .0;

        if let ServerMessage::UsernameError { message } = msg {
            assert!(!message.chars().any(|c| c.is_control()));
            assert_eq!(message, "that username is reserved");
        } else {
            panic!("expected UsernameError, got {:?}", msg);
        }
    }
}
