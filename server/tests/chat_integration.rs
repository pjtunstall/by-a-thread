use renet::{ConnectionConfig, DefaultChannel, RenetServer};

use server::state::{Lobby, ServerState};
use server::{RenetServerNetworkHandle, handle_messages, process_events};
use shared::auth::Passcode;

fn empty_passcode() -> Passcode {
    Passcode {
        bytes: Vec::new(),
        string: String::new(),
    }
}

#[test]
fn chat_messages_are_broadcast_to_other_clients() {
    let mut server = RenetServer::new(ConnectionConfig::default());
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
        lobby.register_username(bob_id, "Bob");
    } else {
        panic!("State should be Lobby");
    }

    alice.send_message(
        DefaultChannel::ReliableOrdered,
        "Hello, Bob!".as_bytes().to_vec(),
    );
    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    let message = bob
        .receive_message(DefaultChannel::ReliableOrdered)
        .expect("Bob should receive the chat message");
    assert_eq!(String::from_utf8_lossy(&message), "Alice: Hello, Bob!");
}

#[test]
fn players_are_notified_when_others_join_and_leave() {
    let mut server = RenetServer::new(ConnectionConfig::default());
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
    } else {
        panic!("State should be Lobby");
    }

    bob.send_message(DefaultChannel::ReliableOrdered, "Bob".as_bytes().to_vec());
    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");
    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");

    let join_message = alice
        .receive_message(DefaultChannel::ReliableOrdered)
        .expect("Alice should be notified when Bob joins");
    assert_eq!(
        String::from_utf8_lossy(&join_message),
        "Bob joined the chat."
    );

    server.disconnect_local_client(bob_id, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");

    let leave_message = alice
        .receive_message(DefaultChannel::ReliableOrdered)
        .expect("Alice should be notified when Bob leaves");
    assert_eq!(
        String::from_utf8_lossy(&leave_message),
        "Bob left the chat."
    );
}

#[test]
fn test_handle_messages_username_success_and_broadcast() {
    let mut server = RenetServer::new(ConnectionConfig::default());
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
    } else {
        panic!("State should be Lobby");
    }

    bob.send_message(DefaultChannel::ReliableOrdered, "Bob".as_bytes().to_vec());
    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");
    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");

    if let ServerState::Lobby(lobby) = &state {
        assert_eq!(lobby.username(2), Some("Bob"));
    } else {
        panic!("State should be Lobby");
    }

    let mut bob_msgs = Vec::new();
    while let Some(message) = bob.receive_message(DefaultChannel::ReliableOrdered) {
        bob_msgs.push(String::from_utf8_lossy(&message).to_string());
    }

    assert!(bob_msgs.contains(&"Welcome, Bob!".to_string()));
    assert!(
        bob_msgs
            .iter()
            .any(|msg| msg.contains("Players online: Alice"))
    );
    assert!(
        !bob_msgs.contains(&"Bob joined the chat.".to_string()),
        "Bob should not be told that he himself joined"
    );

    let alice_msg = alice
        .receive_message(DefaultChannel::ReliableOrdered)
        .expect("Alice should have received a message");
    assert_eq!(String::from_utf8_lossy(&alice_msg), "Bob joined the chat.");
}
