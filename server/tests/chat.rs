use std::time::Duration;

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use renet::{ChannelConfig, ClientNotFound, ConnectionConfig, RenetServer, SendType};

use common::{
    net::AppChannel,
    protocol::{ClientMessage, MAX_CLIENT_MESSAGE_BYTES, ServerMessage},
};
use server::{
    net::RenetServerNetworkHandle,
    run::update_server_state,
    state::{Lobby, ServerState},
};

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

fn receive_until_message(
    client: &mut renet::RenetClient,
    expected: fn(&ServerMessage) -> bool,
) -> Vec<u8> {
    loop {
        let data = client
            .receive_message(AppChannel::ReliableOrdered)
            .expect("expected a message");
        let (msg, _) = decode_from_slice::<ServerMessage, _>(&data, standard())
            .expect("failed to deserialize");
        if expected(&msg) {
            return data.to_vec();
        }
    }
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
        .expect("process Alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, bob) {}
    server.update(tick_duration);
}

#[test]
fn chat_messages_are_broadcast_to_other_clients() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.register_username(alice_id, "alice");
        lobby.register_username(bob_id, "bob");
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
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process Alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, &mut bob) {}

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    let message_data = receive_until_message(&mut bob, |m| matches!(m, ServerMessage::ChatMessage { .. }));
    let message = decode_from_slice::<ServerMessage, _>(&message_data, standard())
        .expect("failed to deserialize message")
        .0;

    let alice_color = if let ServerState::Lobby(lobby) = &state {
        lobby.color(alice_id).expect("missing color for Alice")
    } else {
        panic!("state should be Lobby");
    };

    if let ServerMessage::ChatMessage {
        username,
        color,
        content,
    } = message
    {
        assert_eq!(username, "alice");
        assert_eq!(color, alice_color);
        assert_eq!(content, "Hello, Bob!");
    } else {
        panic!("expected ChatMessage, got {:?}", message);
    }
}

#[test]
fn players_are_notified_when_others_join_and_leave() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.register_username(alice_id, "alice");
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
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process alice failed");
    if let Err(ClientNotFound) = server.process_local_client(2, &mut bob) {}

    alice.update(Duration::from_millis(16));
    bob.update(Duration::from_millis(16));

    let join_data = receive_until_message(&mut alice, |m| matches!(m, ServerMessage::UserJoined { .. }));
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
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
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

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.register_username(alice_id, "alice");
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
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    server.update(Duration::from_millis(16));

    server
        .process_local_client(1, &mut alice)
        .expect("process Alice failed");
    server
        .process_local_client(2, &mut bob)
        .expect("process Bob failed");

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
        bob_msgs.iter().any(|msg| {
            matches!(msg, ServerMessage::Welcome { username, .. } if username == "Bob")
        }),
        "Bob did not receive a welcome message"
    );

    let alice_color = if let ServerState::Lobby(lobby) = &state {
        lobby.color(alice_id).expect("missing color for Alice")
    } else {
        panic!("state should be Lobby");
    };

    assert!(
        bob_msgs.iter().any(|msg| {
            matches!(msg, ServerMessage::Roster { online } if online.len() == 1
                && online[0].username == "alice"
                && online[0].color == alice_color)
        }),
        "Bob did not receive a correct roster message"
    );

    assert!(
        !bob_msgs.iter().any(|msg| {
            matches!(msg, ServerMessage::UserJoined { username } if username == "Bob")
        }),
        "Bob should not be told that he himself joined"
    );

    let alice_data = receive_until_message(&mut alice, |m| matches!(m, ServerMessage::UserJoined { .. }));
    let alice_msg = decode_from_slice::<ServerMessage, _>(&alice_data, standard())
        .unwrap()
        .0;

    if let ServerMessage::UserJoined { username } = alice_msg {
        assert_eq!(username, "Bob");
    } else {
        panic!("Alice expected UserJoined message, got {:?}", alice_msg);
    }
}

#[test]
fn oversized_message_disconnects_client() {
    let mut server = setup_test_server();
    let mut state = ServerState::Lobby(Lobby::new());

    let alice_id = 1;
    let bob_id = 2;
    let mut alice = server.new_local_client(alice_id);
    let mut bob = server.new_local_client(bob_id);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    full_tick(&mut server, &mut alice, &mut bob);

    if let ServerState::Lobby(lobby) = &mut state {
        lobby.register_username(alice_id, "alice");
        lobby.register_username(bob_id, "bob");
    } else {
        panic!("state should be Lobby");
    }

    let oversize_content = "x".repeat(MAX_CLIENT_MESSAGE_BYTES);
    let msg = ClientMessage::SendChat(oversize_content);
    let payload = encode_to_vec(&msg, standard()).expect("failed to serialize message");
    assert!(
        payload.len() > MAX_CLIENT_MESSAGE_BYTES,
        "test payload must exceed limit"
    );
    alice.send_message(AppChannel::ReliableOrdered, payload);

    full_tick(&mut server, &mut alice, &mut bob);

    {
        let mut network_handle = RenetServerNetworkHandle {
            server: &mut server,
        };
        let mut any_client_ever_connected = false;
        update_server_state(
            &mut network_handle,
            &mut state,
            &mut any_client_ever_connected,
        );
    }

    assert!(
        !server.clients_id().contains(&alice_id),
        "Alice should be disconnected for oversized message"
    );
}
