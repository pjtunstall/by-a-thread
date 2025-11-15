use std::time::Duration;

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use renet::{ChannelConfig, ClientNotFound, ConnectionConfig, RenetServer, SendType};

use server::{
    net::RenetServerNetworkHandle,
    run::process_events,
    state::{Lobby, ServerState},
    state_handlers::handle_messages,
};
use shared::{
    auth::Passcode,
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

fn empty_passcode() -> Passcode {
    Passcode {
        bytes: Vec::new(),
        string: String::new(),
    }
}

fn setup_test_server() -> RenetServer {
    let reliable_config = ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(100),
        },
    };

    let unreliable_config = ChannelConfig {
        channel_id: 1,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };

    let time_sync_config = ChannelConfig {
        channel_id: 2,
        max_memory_usage_bytes: 1 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };

    let client_channels_config = vec![
        reliable_config.clone(),
        unreliable_config.clone(),
        time_sync_config.clone(),
    ];
    let server_channels_config = vec![reliable_config, unreliable_config, time_sync_config];

    let connection_config = ConnectionConfig {
        client_channels_config,
        server_channels_config,
        ..Default::default()
    };

    RenetServer::new(connection_config)
}

fn full_tick(
    server: &mut RenetServer,
    alice: &mut renet::RenetClient,
    bob: &mut renet::RenetClient,
) {
    let tick_duration = Duration::from_millis(16);
    alice.update(tick_duration);
    bob.update(tick_duration);
    server
        .process_local_client(1, alice)
        .expect("process alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, bob) {}
    server.update(tick_duration);
}

#[test]
fn chat_messages_are_broadcast_to_other_clients() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
        lobby.register_username(bob_id, "Bob");
    } else {
        panic!("state should be Lobby");
    }

    let msg = ClientMessage::SendChat("Hello, Bob!".to_string());
    let payload = encode_to_vec(&msg, standard()).expect("failed to serialize message");
    alice.send_message(AppChannel::ReliableOrdered, payload);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, &mut bob) {}

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    let message_data = bob
        .receive_message(AppChannel::ReliableOrdered)
        .expect("Bob should receive the chat message");
    let message = decode_from_slice::<ServerMessage, _>(&message_data, standard())
        .expect("failed to deserialize message")
        .0;

    if let ServerMessage::ChatMessage { username, content } = message {
        assert_eq!(username, "Alice");
        assert_eq!(content, "Hello, Bob!");
    } else {
        panic!("expected ChatMessage, got {:?}", message);
    }
}

#[test]
fn players_are_notified_when_others_join_and_leave() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
    } else {
        panic!("state should be Lobby");
    }

    let msg = ClientMessage::SetUsername("Bob".to_string());
    let payload = encode_to_vec(&msg, standard()).expect("failed to serialize message");
    bob.send_message(AppChannel::ReliableOrdered, payload);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, &mut bob) {}

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    let join_data = alice
        .receive_message(AppChannel::ReliableOrdered)
        .expect("Alice should be notified when Bob joins");
    let join_message = decode_from_slice::<ServerMessage, _>(&join_data, standard())
        .expect("failed to deserialize join message")
        .0;

    if let ServerMessage::UserJoined { username } = join_message {
        assert_eq!(username, "Bob");
    } else {
        panic!("expected UserJoined message, got {:?}", join_message);
    }

    server.disconnect_local_client(bob_id, &mut bob);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, &mut bob) {}

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    let leave_data = alice
        .receive_message(AppChannel::ReliableOrdered)
        .expect("Alice should be notified when Bob leaves");
    let leave_message = decode_from_slice::<ServerMessage, _>(&leave_data, standard())
        .expect("failed to deserialize leave message")
        .0;

    if let ServerMessage::UserLeft { username } = leave_message {
        assert_eq!(username, "Bob");
    } else {
        panic!("expected UserLeft message, got {:?}", leave_message);
    }
}

#[test]
fn test_handle_messages_username_success_and_broadcast() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());
    let passcode = empty_passcode();

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        process_events(&mut network_handle, &mut state);
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.mark_authenticated(alice_id);
        lobby.register_username(alice_id, "Alice");
        lobby.mark_authenticated(bob_id);
    } else {
        panic!("state should be Lobby");
    }

    let msg = ClientMessage::SetUsername("Bob".to_string());
    let payload = encode_to_vec(&msg, standard()).expect("failed to serialize message");
    bob.send_message(AppChannel::ReliableOrdered, payload);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        handle_messages(&mut network_handle, &mut state, &passcode);
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process alice failed");
    server
        .process_local_client(2, &mut bob)
        .expect("process bob failed");

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    if let ServerState::Lobby(lobby) = &state {
        assert_eq!(lobby.username(2), Some("Bob"));
    } else {
        panic!("state should be Lobby");
    }

    let mut bob_msgs = Vec::new();
    while let Some(message_data) = bob.receive_message(AppChannel::ReliableOrdered) {
        let msg = decode_from_slice::<ServerMessage, _>(&message_data, standard())
            .unwrap()
            .0;
        bob_msgs.push(msg);
    }

    assert!(
        bob_msgs
            .iter()
            .any(|msg| { matches!(msg, ServerMessage::Welcome { username } if username == "Bob") }),
        "Bob did not receive a Welcome message"
    );

    assert!(
        bob_msgs.iter().any(|msg| {
            matches!(msg, ServerMessage::Roster { online } if online == &vec!["Alice".to_string()])
        }),
        "Bob did not receive a correct Roster message"
    );

    assert!(
        !bob_msgs.iter().any(|msg| {
            matches!(msg, ServerMessage::UserJoined { username } if username == "Bob")
        }),
        "Bob should not be told that he himself joined"
    );

    let alice_data = alice
        .receive_message(AppChannel::ReliableOrdered)
        .expect("Alice should have received a message");
    let alice_msg = decode_from_slice::<ServerMessage, _>(&alice_data, standard())
        .unwrap()
        .0;

    if let ServerMessage::UserJoined { username } = alice_msg {
        assert_eq!(username, "Bob");
    } else {
        panic!("Alice expected UserJoined message, got {:?}", alice_msg);
    }
}
