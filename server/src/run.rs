// server/src/lib.rs
use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use bincode::config::standard;
use bincode::serde::encode_to_vec;
use renet::RenetServer;
use renet_netcode::NetcodeServerTransport;

use crate::{
    net::{self, RenetServerNetworkHandle, ServerNetworkEvent, ServerNetworkHandle},
    state::{
        AuthAttemptOutcome, Countdown, Lobby, MAX_AUTH_ATTEMPTS, ServerState,
        evaluate_passcode_attempt,
    },
};
use shared::{
    self,
    auth::Passcode,
    chat::{MAX_CHAT_MESSAGE_BYTES, MAX_USERNAME_LENGTH, UsernameError, sanitize_username},
    maze::{self, maker::Algorithm},
    net::AppChannel,
    protocol::ServerMessage,
};

pub fn run_server() {
    let private_key = shared::auth::private_key();
    let server_addr = net::server_address();
    let socket = net::bind_socket(server_addr);

    let current_time = shared::time::now();
    let protocol_id = shared::protocol::version();

    let server_config =
        net::build_server_config(current_time, protocol_id, server_addr, private_key);
    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("failed to create transport");
    let connection_config = shared::net::connection_config();
    let mut server = RenetServer::new(connection_config);
    let passcode = Passcode::generate(6);
    let mut state = ServerState::Lobby(Lobby::new());

    print_server_banner(protocol_id, server_addr, &passcode);
    server_loop(&mut server, &mut transport, &mut state, &passcode);
    println!("Server shuttig down.");
}

fn print_server_banner(protocol_id: u64, server_addr: SocketAddr, passcode: &Passcode) {
    println!("  Game version:   {}", protocol_id);
    println!("  Server address: {}", server_addr);
    println!("  Passcode:       {}", passcode.string);
}

fn server_loop(
    server: &mut RenetServer,
    transport: &mut NetcodeServerTransport,
    state: &mut ServerState,
    passcode: &Passcode,
) {
    let mut last_updated = Instant::now();
    let mut last_sync_time = Instant::now();
    let sync_interval = Duration::from_millis(50);

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        transport
            .update(duration, server)
            .expect("failed to update transport");
        server.update(duration);

        let mut network_handle = RenetServerNetworkHandle { server };

        process_events(&mut network_handle, state);

        if now.duration_since(last_sync_time) > sync_interval {
            sync_clocks(&mut network_handle);
            last_sync_time = now;
        }

        let next_state = handle_messages(&mut network_handle, state, passcode);
        if let Some(new_state) = next_state {
            *state = new_state;
        }

        transport.send_packets(server);
        thread::sleep(Duration::from_millis(16));
    }
}

pub fn process_events(network: &mut dyn ServerNetworkHandle, state: &mut ServerState) {
    while let Some(event) = network.get_event() {
        match event {
            ServerNetworkEvent::ClientConnected { client_id } => {
                println!("Client {} connected.", client_id);
                state.register_connection(client_id);
            }
            ServerNetworkEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {} disconnected: {}.", client_id, reason);
                state.remove_client(client_id, network);
            }
        }
    }
}

fn sync_clocks(network: &mut dyn ServerNetworkHandle) {
    let server_time_f64 = shared::time::now().as_secs_f64();
    let message = ServerMessage::ServerTime(server_time_f64);
    let payload = encode_to_vec(&message, standard()).expect("Failed to serialize ServerTime");
    network.broadcast_message(AppChannel::ServerTime, payload);
}

pub fn handle_messages(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ServerState,
    passcode: &Passcode,
) -> Option<ServerState> {
    match state {
        ServerState::Lobby(lobby_state) => handle_lobby(network, lobby_state, passcode),
        ServerState::Countdown(countdown_state) => handle_countdown(network, countdown_state),
        _ => {
            todo!();
        }
    }
}

fn handle_countdown(
    _network: &mut dyn ServerNetworkHandle,
    state: &mut Countdown,
) -> Option<ServerState> {
    let server_time = Instant::now();

    if server_time > state.end_time {
        let number = 1;
        let generator = match number {
            1 => Algorithm::Backtrack,
            2 => Algorithm::Wilson,
            _ => Algorithm::Prim,
        };
        let maze = maze::Maze::new(generator);
        println!("{}", maze.log());
        println!("Time up.");
        std::thread::sleep(Duration::from_secs(1));
        std::process::exit(0);
    } else {
        None
    }
}

fn send_username_error(network: &mut dyn ServerNetworkHandle, client_id: u64, message: &str) {
    let message = ServerMessage::UsernameError {
        message: message.to_string(),
    };
    let payload = encode_to_vec(&message, standard()).expect("Failed to serialize UsernameError");
    network.send_message(client_id, AppChannel::ReliableOrdered, payload);
}

fn handle_lobby(
    network: &mut dyn ServerNetworkHandle,
    state: &mut Lobby,
    passcode: &Passcode,
) -> Option<ServerState> {
    for client_id in network.clients_id() {
        while let Some(message) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            if message.len() > MAX_CHAT_MESSAGE_BYTES {
                println!(
                    "Client {} sent an overly long message; ignoring.",
                    client_id
                );
                continue;
            }
            if state.is_authenticating(client_id) {
                let (outcome, attempts_count) = {
                    let attempts_entry = state
                        .authentication_attempts(client_id)
                        .expect("expected authentication state for client");
                    let outcome = evaluate_passcode_attempt(
                        passcode.bytes.as_slice(),
                        attempts_entry,
                        message.as_ref(),
                        MAX_AUTH_ATTEMPTS,
                    );
                    let count = *attempts_entry;
                    (outcome, count)
                };

                match outcome {
                    AuthAttemptOutcome::Authenticated => {
                        println!("Client {} authenticated successfully.", client_id);
                        state.mark_authenticated(client_id);

                        let prompt = format!(
                            "Authentication successful! Please enter a username (1-{} characters).",
                            MAX_USERNAME_LENGTH
                        );
                        let message = ServerMessage::ServerInfo { message: prompt };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize ServerInfo");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                    }
                    AuthAttemptOutcome::TryAgain => {
                        println!(
                            "Client {} sent wrong passcode (Attempt {}).",
                            client_id, attempts_count
                        );

                        let message = ServerMessage::ServerInfo {
                            message: "Incorrect passcode. Try again.".to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize ServerInfo");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                    }
                    AuthAttemptOutcome::Disconnect => {
                        println!("Client {} failed authentication. Disconnecting.", client_id);
                        let message = ServerMessage::ServerInfo {
                            message: "Incorrect passcode. Disconnecting.".to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize ServerInfo");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                        network.disconnect(client_id);
                        state.remove_client(client_id, network);
                    }
                }
            } else if state.needs_username(client_id) {
                let text = String::from_utf8_lossy(&message).to_string();

                match sanitize_username(&text) {
                    Ok(username) => {
                        if state.is_username_taken(&username) {
                            send_username_error(network, client_id, "Username is already taken.");
                            continue;
                        }

                        state.register_username(client_id, &username);
                        println!("Client {} set username to '{}'.", client_id, username);

                        let message = ServerMessage::Welcome {
                            username: username.to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize Welcome");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);

                        let others = state.usernames_except(client_id);
                        let message = ServerMessage::Roster { online: others };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize Roster");
                        network.send_message(client_id, AppChannel::ReliableOrdered, payload);

                        if state.usernames_except(client_id).is_empty() {
                            state.set_host(client_id, network);
                        }

                        let message = ServerMessage::UserJoined {
                            username: username.to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize UserJoined");
                        network.broadcast_message_except(
                            client_id,
                            AppChannel::ReliableOrdered,
                            payload,
                        );
                    }
                    Err(err) => {
                        let error_text = match err {
                            UsernameError::Empty => "Username must not be empty.",
                            UsernameError::TooLong => "Username is too long.",
                            UsernameError::InvalidCharacter(_) => {
                                "Username contains invalid characters."
                            }
                        };
                        send_username_error(network, client_id, error_text);
                    }
                }
            } else {
                let text = String::from_utf8_lossy(&message).trim().to_string();
                if text.is_empty() {
                    continue;
                }

                if text == shared::auth::START_COUNTDOWN {
                    if state.is_host(client_id) {
                        let host = state
                            .username(client_id)
                            .expect("host should have a username");
                        println!("Host ({}) started the game.", host);

                        let countdown_duration = Duration::from_secs(11);
                        let end_time_f64 =
                            shared::time::now().as_secs_f64() + countdown_duration.as_secs_f64();

                        let message = ServerMessage::CountdownStarted {
                            end_time: end_time_f64,
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("Failed to serialize CountdownStarted");

                        network.broadcast_message(AppChannel::ReliableOrdered, payload);

                        let end_time_instant = Instant::now() + countdown_duration;

                        return Some(ServerState::Countdown(Countdown::new(
                            state,
                            end_time_instant,
                        )));
                    }
                    continue;
                }

                if let Some(username) = state.username(client_id) {
                    println!("{}: {}", username, text);
                    let message = ServerMessage::ChatMessage {
                        username: username.to_string(),
                        content: text,
                    };
                    let payload = encode_to_vec(&message, standard())
                        .expect("Failed to serialize ChatMessage");
                    network.broadcast_message(AppChannel::ReliableOrdered, payload);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Lobby, ServerState};
    use crate::test_helpers::MockServerNetwork;
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;
    use shared::auth::Passcode;
    use shared::protocol::ServerMessage;

    #[test]
    fn test_process_events_client_connect() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());

        network.queue_event(ServerNetworkEvent::ClientConnected { client_id: 1 });

        process_events(&mut network, &mut state);

        if let ServerState::Lobby(lobby) = state {
            assert!(lobby.is_authenticating(1));
        } else {
            panic!("State is not Lobby");
        }
    }

    #[test]
    fn test_process_events_client_disconnect_with_username() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());

        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(1);
            lobby.mark_authenticated(1);
            lobby.register_username(1, "Alice");
        }

        network.queue_event(ServerNetworkEvent::ClientDisconnected {
            client_id: 1,
            reason: "timeout".to_string(),
        });

        process_events(&mut network, &mut state);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(1), None);
        } else {
            panic!("State is not Lobby");
        }

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::UserLeft { username } = msg {
            assert_eq!(username, "Alice");
        } else {
            panic!("Expected UserLeft message, got {:?}", msg);
        }
    }

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

        network.queue_raw_message(1, vec![1, 2, 3, 4, 5, 6]);

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert!(lobby.needs_username(1));
        } else {
            panic!("State is not Lobby");
        }

        let client_msgs = network.get_sent_messages_data(1);
        assert_eq!(client_msgs.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&client_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert!(message.starts_with("Authentication successful!"));
        } else {
            panic!("Expected ServerInfo message, got {:?}", msg);
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
            network.queue_message(1, "000000");
        }

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(1), None);
        } else {
            panic!("State is not Lobby");
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
            panic!("Expected ServerInfo message, got {:?}", msg);
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

        network.queue_message(2, "Bob");

        let _ = handle_messages(&mut network, &mut state, &passcode);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(2), Some("Bob"));
        } else {
            panic!("State is not Lobby");
        }

        let bob_msgs = network.get_sent_messages_data(2);
        assert_eq!(bob_msgs.len(), 2);

        let msg1 = decode_from_slice::<ServerMessage, _>(&bob_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::Welcome { username } = msg1 {
            assert_eq!(username, "Bob");
        } else {
            panic!("Expected Welcome message, got {:?}", msg1);
        }

        let msg2 = decode_from_slice::<ServerMessage, _>(&bob_msgs[1], standard())
            .unwrap()
            .0;
        if let ServerMessage::Roster { online } = msg2 {
            assert_eq!(online, vec!["Alice"]);
        } else {
            panic!("Expected Roster message, got {:?}", msg2);
        }

        let alice_msgs = network.get_sent_messages_data(1);
        assert_eq!(alice_msgs.len(), 1);
        let msg_alice = decode_from_slice::<ServerMessage, _>(&alice_msgs[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::UserJoined { username } = msg_alice {
            assert_eq!(username, "Bob");
        } else {
            panic!("Expected UserJoined message, got {:?}", msg_alice);
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

        network.queue_message(1, "Hello Bob!");

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
            panic!("Expected ChatMessage, got {:?}", msg);
        }
    }
}
