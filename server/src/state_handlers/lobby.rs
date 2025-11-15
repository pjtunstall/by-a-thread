use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    net::ServerNetworkHandle,
    state::{
        AuthAttemptOutcome, ChoosingDifficulty, Lobby, MAX_AUTH_ATTEMPTS, ServerState,
        evaluate_passcode_attempt,
    },
};
use shared::{
    self,
    auth::Passcode,
    chat::{MAX_CHAT_MESSAGE_BYTES, MAX_USERNAME_LENGTH, UsernameError, sanitize_username},
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle_lobby(
    network: &mut dyn ServerNetworkHandle,
    state: &mut Lobby,
    passcode: &Passcode,
) -> Option<ServerState> {
    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!("Client {} sent malformed data. Disconnecting.", client_id);
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::SendPasscode(guess_bytes) => {
                    if !state.is_authenticating(client_id) {
                        eprintln!("Client {} sent passcode in wrong state.", client_id);
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
                                .expect("failed to serialize ServerInfo");
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
                                .expect("failed to serialize ServerInfo");
                            network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                        }
                        AuthAttemptOutcome::Disconnect => {
                            println!("Client {} failed authentication. Disconnecting.", client_id);
                            let message = ServerMessage::ServerInfo {
                                message: "Incorrect passcode. Disconnecting.".to_string(),
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
                        eprintln!("Client {} sent username in wrong state.", client_id);
                        continue;
                    }

                    match sanitize_username(&username_text) {
                        Ok(username) => {
                            if state.is_username_taken(&username) {
                                send_username_error(
                                    network,
                                    client_id,
                                    "Username is already taken.",
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
                                UsernameError::Empty => "Username must not be empty.",
                                UsernameError::TooLong => "Username is too long.",
                                UsernameError::InvalidCharacter(_) => {
                                    "Username contains invalid characters."
                                }
                            };
                            send_username_error(network, client_id, error_text);
                        }
                    }
                }
                ClientMessage::SendChat(content) => {
                    if let Some(username) = state.username(client_id) {
                        if content.is_empty() {
                            continue;
                        }
                        if content.len() > MAX_CHAT_MESSAGE_BYTES {
                            println!(
                                "Client {} sent an overly long chat message; ignoring.",
                                client_id
                            );
                            continue;
                        }

                        println!("{}: {}", username, content);
                        let message = ServerMessage::ChatMessage {
                            username: username.to_string(),
                            content,
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize ChatMessage");
                        network.broadcast_message(AppChannel::ReliableOrdered, payload);
                    } else {
                        eprintln!("Client {} sent chat message in wrong state.", client_id);
                    }
                }
                ClientMessage::RequestStartGame => {
                    if state.is_host(client_id) {
                        return Some(ServerState::ChoosingDifficulty(ChoosingDifficulty::new(
                            state,
                        )));
                    } else {
                        eprintln!("Client {}, not host, tried to start game.", client_id);
                    }
                }
                ClientMessage::SetDifficulty(_) => {
                    eprintln!(
                        "Client {} sent SetDifficulty in Lobby state. Ignoring.",
                        client_id
                    );
                }
            }
        }
    }

    None
}

fn send_username_error(network: &mut dyn ServerNetworkHandle, client_id: u64, message: &str) {
    let message = ServerMessage::UsernameError {
        message: message.to_string(),
    };
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize UsernameError");
    network.send_message(client_id, AppChannel::ReliableOrdered, payload);
}
