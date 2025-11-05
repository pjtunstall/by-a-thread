use renet::{ConnectionConfig, DefaultChannel, RenetServer};
use server::state::ServerState;
use server::{handle_messages, process_events};
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
    let mut state = ServerState::new();
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    process_events(&mut server, &mut state);

    state.mark_authenticated(alice_id);
    state.register_username(alice_id, "Alice");

    state.mark_authenticated(bob_id);
    state.register_username(bob_id, "Bob");

    alice.send_message(
        DefaultChannel::ReliableOrdered,
        "Hello, Bob!".as_bytes().to_vec(),
    );
    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");

    handle_messages(&mut server, &mut state, &passcode);

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
    let mut state = ServerState::new();
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    process_events(&mut server, &mut state);

    state.mark_authenticated(alice_id);
    state.register_username(alice_id, "Alice");

    state.mark_authenticated(bob_id);

    bob.send_message(DefaultChannel::ReliableOrdered, "Bob".as_bytes().to_vec());
    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    handle_messages(&mut server, &mut state, &passcode);

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

    process_events(&mut server, &mut state);

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
fn newcomer_receives_roster_before_join_announcement() {
    let mut server = RenetServer::new(ConnectionConfig::default());
    let mut state = ServerState::new();
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    process_events(&mut server, &mut state);

    state.mark_authenticated(alice_id);
    state.register_username(alice_id, "Alice");

    state.mark_authenticated(bob_id);

    bob.send_message(DefaultChannel::ReliableOrdered, "Bob".as_bytes().to_vec());
    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    handle_messages(&mut server, &mut state, &passcode);

    server
        .process_local_client(bob_id, &mut bob)
        .expect("local client processing should succeed");

    let mut received = Vec::new();
    while let Some(message) = bob.receive_message(DefaultChannel::ReliableOrdered) {
        received.push(String::from_utf8_lossy(&message).to_string());
    }

    assert_eq!(
        received.len(),
        3,
        "expected welcome, roster, and join messages"
    );
    assert_eq!(received[0], "Welcome, Bob!");
    assert!(
        received[1].starts_with("Players online: "),
        "roster list should follow the welcome"
    );
    assert!(
        received[1].contains("Alice"),
        "existing players should appear in the roster"
    );
    assert_eq!(received[2], "Bob joined the chat.");

    // ensure existing players still learn about the newcomer
    server
        .process_local_client(alice_id, &mut alice)
        .expect("local client processing should succeed");
    let join_for_alice = alice
        .receive_message(DefaultChannel::ReliableOrdered)
        .expect("Alice should receive the join announcement");
    assert_eq!(
        String::from_utf8_lossy(&join_for_alice),
        "Bob joined the chat."
    );
}
