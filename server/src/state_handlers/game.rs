use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use glam::Vec3;

use crate::{
    input,
    net::ServerNetworkHandle,
    player::Status,
    state::{Game, ServerState},
};
use common::{
    bullets::{self, Bullet, check_player_collision, update_bullet_position},
    chat::MAX_CHAT_MESSAGE_BYTES,
    constants::{ESCAPE_DURATION, TICKS_PER_BROADCAST},
    input::sanitize,
    net::AppChannel,
    protocol::{BulletEvent, ClientMessage, ServerMessage},
    ring::WireItem,
    snapshot::Snapshot,
    time,
};

// TODO: Consider if any of this logic belongs with the `Game` struct in `server/src/state.rs`.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    handle_reliable_messages(network, state);
    input::receive_inputs(network, state);

    if !state.escape_active {
        let alive_count = state
            .players
            .iter()
            .filter(|p| matches!(p.status, Status::Alive))
            .count();

        if alive_count == 1 {
            trigger_escape(network, state);
        }
    }

    if state.escape_active {
        check_escape_timer_expiration(network, state);
    }

    let player_positions: Vec<(usize, Vec3)> = state
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.status, crate::player::Status::Alive))
        .map(|(i, p)| (i, p.state.position))
        .collect();

    for player in &mut state.players {
        if let Some(&input) = player.input_buffer.get(state.current_tick) {
            player.last_input = input;
        }

        let input = player.last_input;

        if matches!(player.status, crate::player::Status::Alive) {
            player
                .state
                .update(&state.maze, &input, player.index, &player_positions, 1.0);
        }
        player.input_buffer.advance_tail(state.current_tick);
    }

    let mut bullet_events = Vec::new();

    for (player_index, player) in state.players.iter_mut().enumerate() {
        if !matches!(player.status, crate::player::Status::Alive) {
            continue;
        }

        if !player.last_input.fire {
            continue;
        }

        let Some(fire_nonce) = player.last_input.fire_nonce else {
            continue;
        };
        player.last_input.fire_nonce = None;

        let cooldown_ticks = bullets::cooldown_ticks();
        let can_fire = player
            .last_fire_tick
            .map(|tick| state.current_tick.saturating_sub(tick) >= cooldown_ticks)
            .unwrap_or(true);

        if !can_fire || player.bullets_in_air >= bullets::MAX_BULLETS_PER_PLAYER {
            continue;
        }

        let direction = bullets::direction_from_yaw_pitch(player.state.yaw, player.state.pitch);
        if direction == glam::Vec3::ZERO {
            continue;
        }

        let position = bullets::spawn_position(player.state.position, direction);
        let velocity = direction * bullets::SPEED;
        let bullet_id = state.next_bullet_id;
        state.next_bullet_id = state.next_bullet_id.wrapping_add(1);

        state.bullets.push(Bullet::new(
            bullet_id,
            player_index,
            position,
            velocity,
            state.current_tick,
        ));
        player.last_fire_tick = Some(state.current_tick);
        player.bullets_in_air += 1;

        bullet_events.push(BulletEvent::Spawn {
            bullet_id,
            tick: state.current_tick,
            position,
            velocity,
            fire_nonce: Some(fire_nonce),
            shooter_index: player_index,
        });
    }

    update_bullets(state, &mut bullet_events);

    if !bullet_events.is_empty() {
        for event in bullet_events {
            let message = ServerMessage::BulletEvent(event);
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize bullet event");
            let payload_len = payload.len();
            let recipients: Vec<u64> = state
                .client_id_to_index
                .keys()
                .copied()
                .filter(|client_id| !state.after_game_chat_clients.contains(client_id))
                .collect();
            let recipients_count = recipients.len();
            for client_id in recipients {
                network.send_message(client_id, AppChannel::ReliableOrdered, payload.clone());
            }
            state.note_egress_bytes(payload_len.saturating_mul(recipients_count));
        }
    }

    // Only send snapshots every third tick.
    if state.current_tick % TICKS_PER_BROADCAST == 0 {
        let mut egress_bytes = 0usize;
        for (&client_id, &player_index) in &state.client_id_to_index {
            if state.after_game_chat_clients.contains(&client_id) {
                continue;
            }
            let snapshot = state.snapshot_for(player_index);
            let message = ServerMessage::Snapshot(WireItem::<Snapshot> {
                id: state.current_tick as u16,
                data: snapshot,
            });
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize ServerTime");
            egress_bytes = egress_bytes.saturating_add(payload.len());
            network.send_message(client_id, AppChannel::Unreliable, payload);
        }
        state.note_egress_bytes(egress_bytes);
    }

    state.current_tick += 1;
    // // Uncomment to log egress and ingress rates.
    // state.net_stats.log_if_ready();

    None
}

fn handle_reliable_messages(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    for client_id in network.clients_id() {
        let mut ingress_bytes = 0usize;
        while let Some(data) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            ingress_bytes = ingress_bytes.saturating_add(data.len());
            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!(
                    "client {} sent malformed data during game; disconnecting them",
                    client_id
                );
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::EnterAfterGameChat => {
                    let Some(&player_index) = state.client_id_to_index.get(&client_id) else {
                        eprintln!(
                            "client {} entered after-game chat but was not in the game state",
                            client_id
                        );
                        continue;
                    };

                    if !state.after_game_chat_clients.insert(client_id) {
                        continue;
                    }

                    let player = &mut state.players[player_index];
                    if player.exit_tick.is_none() {
                        player.exit_tick = Some(state.current_tick);
                    }

                    let online = state
                        .players
                        .iter()
                        .filter(|player| player.client_id != client_id)
                        .filter(|player| state.after_game_chat_clients.contains(&player.client_id))
                        .map(|player| common::protocol::PlayerRosterEntry {
                            username: player.name.clone(),
                            color: player.color,
                        })
                        .collect::<Vec<_>>();

                    let message = ServerMessage::AfterGameRoster {
                        hades_shades: online,
                    };
                    let payload =
                        encode_to_vec(&message, standard()).expect("failed to serialize Roster");
                    state.note_egress_bytes(payload.len());
                    network.send_message(client_id, AppChannel::ReliableOrdered, payload);

                    let message = ServerMessage::UserJoined {
                        username: state.players[player_index].name.clone(),
                    };
                    let payload = encode_to_vec(&message, standard())
                        .expect("failed to serialize UserJoined");
                    let payload_len = payload.len();
                    let mut egress_bytes = 0usize;

                    for other_id in &state.after_game_chat_clients {
                        if *other_id == client_id {
                            continue;
                        }
                        egress_bytes = egress_bytes.saturating_add(payload_len);
                        network.send_message(
                            *other_id,
                            AppChannel::ReliableOrdered,
                            payload.clone(),
                        );
                    }
                    state.note_egress_bytes(egress_bytes);

                    state.send_leaderboard_if_ready(network);
                }
                ClientMessage::SendChat(content) => {
                    if !state.after_game_chat_clients.contains(&client_id) {
                        eprintln!(
                            "client {} sent chat message during game; ignoring",
                            client_id
                        );
                        continue;
                    }

                    let Some(&player_index) = state.client_id_to_index.get(&client_id) else {
                        continue;
                    };

                    let clean_content = sanitize(&content);
                    let trimmed_content = clean_content.trim();

                    if trimmed_content.is_empty() {
                        continue;
                    }
                    if trimmed_content.len() > MAX_CHAT_MESSAGE_BYTES {
                        println!(
                            "Client {} sent an overly long chat message; ignoring.",
                            client_id
                        );
                        continue;
                    }

                    println!("{}: {}", state.players[player_index].name, trimmed_content);
                    let message = ServerMessage::ChatMessage {
                        username: state.players[player_index].name.clone(),
                        color: state.players[player_index].color,
                        content: trimmed_content.to_string(),
                    };
                    let payload = encode_to_vec(&message, standard())
                        .expect("failed to serialize ChatMessage");
                    let mut egress_bytes = 0usize;
                    for other_id in &state.after_game_chat_clients {
                        egress_bytes = egress_bytes.saturating_add(payload.len());
                        network.send_message(
                            *other_id,
                            AppChannel::ReliableOrdered,
                            payload.clone(),
                        );
                    }
                    state.note_egress_bytes(egress_bytes);
                }
                _ => {}
            }
        }
        state.note_ingress_bytes(ingress_bytes);
    }
}

fn update_bullets(state: &mut Game, events: &mut Vec<BulletEvent>) {
    let mut index = 0;
    while index < state.bullets.len() {
        let mut remove = false;
        let mut hit_inanimate = false;
        let mut hit_player_event = None;

        {
            let bullet = &mut state.bullets[index];

            let update_result = update_bullet_position(bullet, &state.maze, state.current_tick);

            if update_result.should_remove {
                events.push(BulletEvent::Expire {
                    bullet_id: bullet.id,
                    tick: state.current_tick,
                    position: bullet.position,
                    velocity: bullet.velocity,
                });
                remove = true;
            } else {
                hit_inanimate = update_result.hit_inanimate;

                for (player_index, player) in state.players.iter_mut().enumerate() {
                    if !matches!(player.status, crate::player::Status::Alive) {
                        continue;
                    }

                    let collision_result =
                        check_player_collision(bullet, player.state.position, player.health);

                    if collision_result.hit_player {
                        player.health = collision_result.new_health;
                        if collision_result.new_health == 0 {
                            player.status = crate::player::Status::Dead;
                            if player.exit_tick.is_none() {
                                player.exit_tick = Some(state.current_tick);
                            }
                        }

                        if collision_result.should_remove_bullet {
                            remove = true;
                        }

                        hit_player_event = Some(BulletEvent::HitPlayer {
                            bullet_id: bullet.id,
                            tick: state.current_tick,
                            position: bullet.position,
                            velocity: bullet.velocity,
                            target_index: player_index,
                            target_health: collision_result.new_health,
                        });
                        break;
                    }
                }
            }
        }

        if let Some(event) = hit_player_event {
            events.push(event);
        } else if hit_inanimate {
            let bullet = &state.bullets[index];
            events.push(BulletEvent::HitInanimate {
                bullet_id: bullet.id,
                tick: state.current_tick,
                position: bullet.position,
                velocity: bullet.velocity,
            });
        }

        if remove {
            let bullet = state.bullets.swap_remove(index);
            if let Some(shooter) = state.players.get_mut(bullet.shooter_index) {
                shooter.bullets_in_air = shooter.bullets_in_air.saturating_sub(1);
            }
            continue;
        }

        index += 1;
    }
}

fn trigger_escape(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let (exit_z, exit_x) = state.exit_coords;
    state.maze.grid[exit_z][exit_x] = 0;
    state.maze.spaces.push((exit_z, exit_x));
    state.escape_active = true;
    let start_time = time::now_as_secs_f64();
    state.escape_start_time = Some(start_time);

    let message = ServerMessage::EscapeStarted { start_time };
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize EscapeStarted");
    let payload_len = payload.len();
    let recipients: Vec<u64> = state
        .client_id_to_index
        .keys()
        .copied()
        .filter(|client_id| !state.after_game_chat_clients.contains(client_id))
        .collect();
    let recipients_count = recipients.len();
    for client_id in recipients {
        network.send_message(client_id, AppChannel::ReliableOrdered, payload.clone());
    }
    state.note_egress_bytes(payload_len.saturating_mul(recipients_count));

    println!("Sudden death activated! Timer started for 90 seconds.");
}

fn check_escape_timer_expiration(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let Some(start_time) = state.escape_start_time else {
        return;
    };

    let elapsed = time::now_as_secs_f64() - start_time;
    if elapsed < ESCAPE_DURATION as f64 {
        return;
    }

    let current_tick = state.current_tick;
    let mut events = Vec::new();
    let mut total_egress_bytes = 0usize;

    for player in &mut state.players {
        if !matches!(player.status, Status::Alive) {
            continue;
        }

        player.health = 0;
        player.status = Status::Dead;
        if player.exit_tick.is_none() {
            player.exit_tick = Some(current_tick);
        }

        let event = BulletEvent::HitPlayer {
            bullet_id: 0,
            tick: current_tick,
            position: player.state.position,
            velocity: glam::Vec3::ZERO,
            target_index: player.index,
            target_health: 0,
        };
        events.push(event);
    }

    for event in events {
        let message = ServerMessage::BulletEvent(event);
        let payload =
            encode_to_vec(&message, standard()).expect("failed to serialize bullet event");
        let payload_len = payload.len();
        let recipients: Vec<u64> = state
            .client_id_to_index
            .keys()
            .copied()
            .filter(|client_id| !state.after_game_chat_clients.contains(client_id))
            .collect();
        let recipients_count = recipients.len();
        for client_id in recipients {
            network.send_message(client_id, AppChannel::ReliableOrdered, payload.clone());
        }
        total_egress_bytes =
            total_egress_bytes.saturating_add(payload_len.saturating_mul(recipients_count));
    }

    state.note_egress_bytes(total_egress_bytes);
    state.escape_start_time = None;
}
