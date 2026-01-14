use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use glam::Vec3;

use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{
    bullets::{self, Bullet, WallBounce},
    chat::MAX_CHAT_MESSAGE_BYTES,
    constants::TICKS_PER_BROADCAST,
    input::sanitize,
    net::AppChannel,
    protocol::{BulletEvent, ClientMessage, ServerMessage},
    ring::WireItem,
    snapshot::Snapshot,
};

// TODO: Consider if any of this logic belongs with the `Game` struct in `server/src/state.rs`.

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    handle_reliable_messages(network, state);
    input::receive_inputs(network, state);

    let player_positions: Vec<(usize, Vec3)> = state
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.health > 0)
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

        let fire_nonce = player.last_input.fire_nonce;
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
            fire_nonce,
            owner_index: player_index,
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
    state.log_network_stats_if_ready();

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
                        .map(|player| player.name.clone())
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

        {
            let bullet = &mut state.bullets[index];
            bullet.advance(1);

            if bullet.is_expired(state.current_tick) || bullet.has_bounced_enough() {
                events.push(BulletEvent::Expire {
                    bullet_id: bullet.id,
                    tick: state.current_tick,
                    position: bullet.position,
                    velocity: bullet.velocity,
                });
                remove = true;
            } else {
                if bullet.bounce_off_ground() {
                    hit_inanimate = true;
                }

                match bullet.bounce_off_wall(&state.maze) {
                    WallBounce::Bounce => {
                        hit_inanimate = true;
                    }
                    WallBounce::Stuck => {
                        events.push(BulletEvent::Expire {
                            bullet_id: bullet.id,
                            tick: state.current_tick,
                            position: bullet.position,
                            velocity: bullet.velocity,
                        });
                        remove = true;
                    }
                    WallBounce::None => {}
                }
            }
        }

        if remove {
            let bullet = state.bullets.swap_remove(index);
            if let Some(owner) = state.players.get_mut(bullet.owner_index) {
                owner.bullets_in_air = owner.bullets_in_air.saturating_sub(1);
            }
            continue;
        }

        let mut hit_player_event = None;
        {
            let bullet = &mut state.bullets[index];
            for (player_index, player) in state.players.iter_mut().enumerate() {
                if !matches!(player.status, crate::player::Status::Alive) {
                    continue;
                }

                if !bullets::is_bullet_colliding_with_player(bullet.position, player.state.position)
                {
                    continue;
                }

                let new_health = player.health.saturating_sub(1);
                player.health = new_health;
                if new_health == 0 {
                    player.status = crate::player::Status::Dead;
                    if player.exit_tick.is_none() {
                        player.exit_tick = Some(state.current_tick);
                    }
                }

                if new_health > 0 {
                    let delta = player.state.position - bullet.position;
                    let normal = if delta.length_squared() > 0.001 {
                        delta.normalize()
                    } else {
                        -bullet.velocity.normalize_or_zero()
                    };
                    bullet.redirect(normal);
                } else {
                    remove = true;
                }

                hit_player_event = Some(BulletEvent::HitPlayer {
                    bullet_id: bullet.id,
                    tick: state.current_tick,
                    position: bullet.position,
                    velocity: bullet.velocity,
                    target_index: player_index,
                    target_health: new_health,
                });
                break;
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
            if let Some(owner) = state.players.get_mut(bullet.owner_index) {
                owner.bullets_in_air = owner.bullets_in_air.saturating_sub(1);
            }
            continue;
        }

        index += 1;
    }
}
