pub mod countdown;
pub mod difficulty;
pub mod game;
pub mod lobby;

pub use countdown::handle_countdown;
pub use difficulty::handle_choosing_difficulty;
pub use game::handle_in_game;
pub use lobby::handle_lobby;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ChoosingDifficulty, Lobby, MAX_AUTH_ATTEMPTS};
    use crate::test_helpers::MockServerNetwork;
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;
    use bincode::serde::encode_to_vec;
    use shared::auth::Passcode;
    use shared::protocol::{ClientMessage, ServerMessage};

    #[test]
    fn test_handle_lobby_auth_success() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);

        let msg = ClientMessage::SendPasscode(vec![1, 2, 3, 4, 5, 6]);
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle_lobby(&mut network, &mut lobby_state, &passcode);

        assert!(lobby_state.needs_username(1));
        assert!(next_state.is_none());

        let client_msgs = network.get_sent_messages_data(1);
        assert_eq!(client_msgs.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&client_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert!(message.starts_with("Authentication successful!"));
        } else {
            panic!("expected ServerInfo message, got {:?}", msg);
        }
    }

    #[test]
    fn test_handle_lobby_auth_fail_then_disconnect() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        lobby_state.register_connection(1);

        for _ in 0..MAX_AUTH_ATTEMPTS {
            let msg = ClientMessage::SendPasscode(vec![0, 0, 0, 0, 0, 0]);
            let payload = encode_to_vec(&msg, standard()).unwrap();
            network.queue_raw_message(1, payload);
        }

        let next_state = handle_lobby(&mut network, &mut lobby_state, &passcode);

        assert_eq!(lobby_state.username(1), None);
        assert!(next_state.is_none());

        assert!(network.disconnected_clients.contains(&1));
        let client_msgs = network.get_sent_messages_data(1);
        let last_msg_data = client_msgs.last().unwrap();
        let msg = decode_from_slice::<ServerMessage, _>(last_msg_data, standard())
            .unwrap()
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert!(message.starts_with("Incorrect passcode. Disconnecting."));
        } else {
            panic!("expected ServerInfo message, got {:?}", msg);
        }
    }

    #[test]
    fn test_handle_lobby_username_success_and_broadcast() {
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

        let next_state = handle_lobby(&mut network, &mut lobby_state, &passcode);

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
    fn test_handle_lobby_chat_message() {
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

        let next_state = handle_lobby(&mut network, &mut lobby_state, &passcode);

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
    fn test_handle_lobby_chat_sanitization() {
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
        let expected_content = "HelloBob![2J";

        let msg = ClientMessage::SendChat(malicious_content.to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let next_state = handle_lobby(&mut network, &mut lobby_state, &passcode);

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
                "Chat content was not properly sanitized"
            );
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }

    #[test]
    fn test_handle_choosing_difficulty_chat_sanitization() {
        let mut network = MockServerNetwork::new();
        let mut lobby_state = Lobby::new();

        let host_id = 1;
        let user_id = 2;

        network.add_client(host_id);
        lobby_state.register_connection(host_id);
        lobby_state.mark_authenticated(host_id);
        lobby_state.register_username(host_id, "Host");

        network.add_client(user_id);
        lobby_state.register_connection(user_id);
        lobby_state.mark_authenticated(user_id);
        lobby_state.register_username(user_id, "User");

        let mut choosing_state = ChoosingDifficulty::new(&lobby_state);

        let malicious_content = "  Hi\x07Host!\x1B[2J  ";
        let expected_content = "HiHost![2J";

        let msg = ClientMessage::SendChat(malicious_content.to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(user_id, payload);

        let next_state = handle_choosing_difficulty(&mut network, &mut choosing_state);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let (msg, _) = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard()).unwrap();

        if let ServerMessage::ChatMessage { username, content } = msg {
            assert_eq!(username, "User");
            assert_eq!(
                content, expected_content,
                "Chat content was not properly sanitized"
            );
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }
}
