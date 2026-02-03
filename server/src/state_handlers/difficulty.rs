use std::time::{Duration, Instant};

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    net::ServerNetworkHandle,
    state::{ChoosingDifficulty, Countdown, ServerState},
};
use common::{
    self,
    chat::MAX_CHAT_MESSAGE_BYTES,
    net::AppChannel,
    protocol::{ClientMessage, GAME_ALREADY_STARTED_MESSAGE, ServerMessage},
    snapshot::InitialData,
};

pub fn handle(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ChoosingDifficulty,
    last_activity: &mut Instant,
) -> Option<ServerState> {
    let Some(host_id) = state.host_id() else {
        eprintln!("difficulty selection has no host; ignoring inputs");
        return None;
    };

    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            *last_activity = Instant::now();

            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!(
                    "client {} sent malformed data; disconnecting them",
                    client_id
                );
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::SetDifficulty(level) => {
                    if client_id != host_id {
                        eprintln!("non-host {} tried to set difficulty", client_id);
                        continue;
                    }

                    if !(1..=8).contains(&level) {
                        eprintln!("host {} sent invalid difficulty level: {}", host_id, level);
                        let msg = ServerMessage::ServerInfo {
                            message: "Invalid choice. Please press 1 through 8.".to_string(),
                        };
                        let payload = encode_to_vec(&msg, standard()).expect("failed to serialize");
                        network.send_message(host_id, AppChannel::ReliableOrdered, payload);
                        return None;
                    }

                    println!("Host selected difficulty {}.", level);
                    state.set_difficulty(level);

                    let game_data =
                        InitialData::new(&state.lobby.usernames, state.lobby.colors(), level);

                    println!("\n{}", game_data.maze);
                    println!();
                    for player in &game_data.players {
                        println!("{:#?}\n", player);
                    }

                    let countdown_duration = Duration::from_secs(11);
                    let end_time_instant = Instant::now() + countdown_duration;

                    return Some(ServerState::Countdown(Countdown::new(
                        state,
                        end_time_instant,
                        game_data,
                    )));
                }
                ClientMessage::SendChat(content) => {
                    if let Some(username) = state.username(client_id) {
                        let clean_content: String =
                            content.chars().filter(|c| !c.is_control()).collect();

                        let trimmed_content = clean_content.trim();
                        if trimmed_content.is_empty() {
                            continue;
                        }
                        if trimmed_content.len() > MAX_CHAT_MESSAGE_BYTES {
                            continue;
                        }

                        println!("{}: {}", username, trimmed_content);
                        let color = state
                            .lobby
                            .color(client_id)
                            .expect("missing player color for chat");
                        let message = ServerMessage::ChatMessage {
                            username: username.to_string(),
                            color,
                            content: trimmed_content.to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize ChatMessage");
                        network.broadcast_message(AppChannel::ReliableOrdered, payload);
                    }
                }
                ClientMessage::SendPasscode(_) | ClientMessage::SetUsername(_) => {
                    let msg = ServerMessage::ServerInfo {
                        message: GAME_ALREADY_STARTED_MESSAGE.to_string(),
                    };
                    let payload =
                        encode_to_vec(&msg, standard()).expect("failed to serialize ServerInfo");
                    network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                }
                ClientMessage::RequestStartGame => {
                    eprintln!(
                        "client {} asked to choose difficulty, but the host is already doing so; ignoring",
                        client_id
                    );
                }
                ClientMessage::EnterAfterGameChat => {
                    eprintln!(
                        "client {} sent EnterAfterGameChat while choosing difficulty; ignoring",
                        client_id
                    );
                }
                ClientMessage::Input(_) => {
                    eprintln!("client {} sent in-game input; ignoring", client_id);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use bincode::{config::standard, serde::decode_from_slice, serde::encode_to_vec};

    use super::*;
    use crate::{
        state::{ChoosingDifficulty, Lobby},
        test_helpers::MockServerNetwork,
    };
    use common::protocol::{ClientMessage, ServerMessage};

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

        let mut last_activity = Instant::now();
        let next_state = handle(&mut network, &mut choosing_state, &mut last_activity);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let (msg, _) = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard()).unwrap();

        let user_color = choosing_state
            .lobby
            .color(user_id)
            .expect("missing color for user");
        if let ServerMessage::ChatMessage {
            username,
            color,
            content,
        } = msg
        {
            assert_eq!(username, "User");
            assert_eq!(color, user_color);
            assert_eq!(
                content, expected_content,
                "chat content was not properly sanitized"
            );
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }
}
