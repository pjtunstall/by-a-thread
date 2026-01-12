use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{
    bullets::{self, Bullet, WallBounce},
    constants::TICKS_PER_BROADCAST,
    net::AppChannel,
    protocol::{BulletEvent, ServerMessage},
    ring::WireItem,
    snapshot::Snapshot,
};

// TODO: Consider if any of this logic belongs with the `Game` struct iN `server/src/state.rs`.

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    input::receive_inputs(network, state);

    // TODO: Randomize as in input collection for fairness.
    for player in &mut state.players {
        if let Some(&input) = player.input_buffer.get(state.current_tick) {
            player.last_input = input;
        }

        let input = player.last_input;
        // println!("{:?}", input);

        if matches!(player.status, crate::player::Status::Alive) {
            player.state.update(&state.maze, &input);
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

        state
            .bullets
            .push(Bullet::new(bullet_id, player_index, position, velocity, state.current_tick));
        player.last_fire_tick = Some(state.current_tick);
        player.bullets_in_air += 1;

        bullet_events.push(BulletEvent::Spawn {
            bullet_id,
            tick: state.current_tick,
            position,
            velocity,
            fire_nonce,
        });
    }

    update_bullets(state, &mut bullet_events);

    if !bullet_events.is_empty() {
        for event in bullet_events {
            let message = ServerMessage::BulletEvent(event);
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize bullet event");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);
        }
    }

    // Only send snapshots every third tick.
    if state.current_tick % TICKS_PER_BROADCAST == 0 {
        for i in 0..state.players.len() {
            let snapshot = state.snapshot_for(i);
            let message = ServerMessage::Snapshot(WireItem::<Snapshot> {
                id: state.current_tick as u16,
                data: snapshot,
            });
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize ServerTime");
            network.send_message(state.players[i].client_id, AppChannel::Unreliable, payload);
        }
    }

    state.current_tick += 1;
    // println!("{}", state.current_tick);

    None
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
