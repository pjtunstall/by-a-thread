use std::collections::HashMap;
use std::time::{Duration, Instant};

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use rand::random_range;

use crate::{
    net::ServerNetworkHandle,
    state::{ChoosingDifficulty, Countdown, ServerState},
};
use shared::{
    self,
    chat::MAX_CHAT_MESSAGE_BYTES,
    consts::PLAYER_HEIGHT,
    math::Vec3,
    maze::{self, maker::Algorithm},
    net::AppChannel,
    player::Player,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ChoosingDifficulty,
) -> Option<ServerState> {
    let host_id = state.host_id();

    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!("Client {} sent malformed data. Disconnecting.", client_id);
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::SetDifficulty(level) => {
                    if client_id != host_id {
                        eprintln!("Non-host {} tried to set difficulty.", client_id);
                        continue;
                    }

                    if !(1..=3).contains(&level) {
                        eprintln!("Host {} sent invalid difficulty level: {}", host_id, level);
                        let msg = ServerMessage::ServerInfo {
                            message: "Invalid choice. Please press 1, 2, or 3.".to_string(),
                        };
                        let payload = encode_to_vec(&msg, standard()).expect("failed to serialize");
                        network.send_message(host_id, AppChannel::ReliableOrdered, payload);
                        return None;
                    }

                    println!("Host selected difficulty {}.", level);
                    state.set_difficulty(level);

                    let generator = match level {
                        1 => Algorithm::Backtrack,
                        2 => Algorithm::Wilson,
                        _ => Algorithm::Prim,
                    };
                    let maze = maze::Maze::new(generator);
                    let maze_layout = maze.log();
                    println!("\n{}", maze_layout);

                    let countdown_duration = Duration::from_secs(11);
                    let end_time_instant = Instant::now() + countdown_duration;

                    println!();

                    let mut spaces_remaining = maze.spaces.clone();
                    let mut player_count: usize = 0;
                    let players: HashMap<u64, Player> = state
                        .lobby
                        .usernames
                        .clone()
                        .into_iter()
                        .map(|(id, username)| {
                            let space_index = random_range(0..spaces_remaining.len());
                            let (y, x) = spaces_remaining.remove(space_index);
                            let start_position = maze
                                .position_from_grid_coordinates(PLAYER_HEIGHT, y, x)
                                .expect("failed to get start position from maze");
                            let player = Player {
                                id,
                                name: username.clone(),
                                position: start_position,
                                orientation: Vec3::ZERO,
                                color: shared::player::COLORS
                                    [player_count % shared::player::COLORS.len()],
                            };
                            player_count += 1;
                            println!("{:#}", player);
                            (id, player)
                        })
                        .collect();

                    return Some(ServerState::Countdown(Countdown::new(
                        state,
                        players,
                        end_time_instant,
                        maze,
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
                        let message = ServerMessage::ChatMessage {
                            username: username.to_string(),
                            content: trimmed_content.to_string(),
                        };
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize ChatMessage");
                        network.broadcast_message(AppChannel::ReliableOrdered, payload);
                    }
                }
                _ => {
                    eprintln!(
                        "Client {} sent unexpected message in difficulty choice state.",
                        client_id
                    );
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
    use shared::protocol::{ClientMessage, ServerMessage};

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

        let next_state = handle(&mut network, &mut choosing_state);

        assert!(next_state.is_none());

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let (msg, _) = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard()).unwrap();

        if let ServerMessage::ChatMessage { username, content } = msg {
            assert_eq!(username, "User");
            assert_eq!(
                content, expected_content,
                "chat content was not properly sanitized"
            );
        } else {
            panic!("expected ChatMessage, got {:?}", msg);
        }
    }
}
