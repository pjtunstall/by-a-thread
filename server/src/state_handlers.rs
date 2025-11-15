pub mod countdown;
pub mod difficulty;
pub mod lobby;

pub use countdown::handle_countdown;
pub use difficulty::handle_choosing_difficulty;
pub use lobby::handle_lobby;

use crate::{net::ServerNetworkHandle, state::ServerState};
use shared::{self, auth::Passcode};

pub fn handle_messages(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ServerState,
    passcode: &Passcode,
) -> Option<ServerState> {
    match state {
        ServerState::Lobby(lobby_state) => handle_lobby(network, lobby_state, passcode),
        ServerState::ChoosingDifficulty(difficulty_state) => {
            handle_choosing_difficulty(network, difficulty_state)
        }
        ServerState::Countdown(countdown_state) => handle_countdown(network, countdown_state),
        ServerState::InGame(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Lobby, MAX_AUTH_ATTEMPTS, ServerState};
    use crate::test_helpers::MockServerNetwork;
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;
    use bincode::serde::encode_to_vec;
    use shared::auth::Passcode;
    use shared::protocol::{ClientMessage, ServerMessage};

    #[test]
    fn test_handle_messages_auth_success() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(1);
        }

        let msg = ClientMessage::SendPasscode(vec![1, 2, 3, 4, 5, 6]);
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert!(lobby.needs_username(1));
        } else {
            panic!("state is not Lobby");
        }

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
    fn test_handle_messages_auth_fail_then_disconnect() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(1);
        }

        for _ in 0..MAX_AUTH_ATTEMPTS {
            let msg = ClientMessage::SendPasscode(vec![0, 0, 0, 0, 0, 0]);
            let payload = encode_to_vec(&msg, standard()).unwrap();
            network.queue_raw_message(1, payload);
        }

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(1), None);
        } else {
            panic!("state is not Lobby");
        }

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
    fn test_handle_messages_username_success_and_broadcast() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        network.add_client(1);
        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(1);
            lobby.mark_authenticated(1);
            lobby.register_username(1, "Alice");
        }

        network.add_client(2);
        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(2);
            lobby.mark_authenticated(2);
        }

        let msg = ClientMessage::SetUsername("Bob".to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(2, payload);

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(2), Some("Bob"));
        } else {
            panic!("state is not Lobby");
        }

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
    fn test_handle_messages_chat_message() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());
        let passcode =
            Passcode::from_string("123456").expect("failed to create passcode from string");

        if let ServerState::Lobby(lobby) = &mut state {
            network.add_client(1);
            lobby.register_connection(1);
            lobby.mark_authenticated(1);
            lobby.register_username(1, "Alice");

            network.add_client(2);
            lobby.register_connection(2);
            lobby.mark_authenticated(2);
            lobby.register_username(2, "Bob");
        }

        let msg = ClientMessage::SendChat("Hello Bob!".to_string());
        let payload = encode_to_vec(&msg, standard()).unwrap();
        network.queue_raw_message(1, payload);

        let _ = handle_messages(&mut network, &mut state, &passcode);

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
}
