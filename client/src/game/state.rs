use std::{
    fmt,
    time::{Duration, Instant},
};

use ::rand::{Rng, rng};
use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use macroquad::prelude::*;

use crate::{
    assets::Assets,
    fade::{self, Fade},
    frame::FrameRate,
    game::input,
    game::world::maze::{MazeExtension, MazeMeshes},
    info,
    net::NetworkHandle,
    session::Clock,
    state::ClientState,
    time::INTERPOLATION_DELAY_SECS,
};
use common::{
    bullets,
    constants::{INPUT_HISTORY_LENGTH, SNAPSHOT_BUFFER_LENGTH, TICK_SECS, TICK_SECS_F32},
    maze::Maze,
    net::AppChannel,
    player::{self, Player, PlayerInput},
    protocol::{BulletEvent, ClientMessage, ServerMessage},
    ring::WireItem,
    ring::{NetworkBuffer, Ring},
    snapshot::{InitialData, Snapshot},
};

// A guard against getting stuck in loop receiving snapshots from server if
// messages are coming faster than we can drain the queue.
const NETWORK_TIME_BUDGET: Duration = Duration::from_millis(2);
const BULLET_COLOR_MODE: BulletColorMode = BulletColorMode::FadeToRed;
const OBE_TIME_STEP: f32 = 1.0 / 60.0;
const OBE_SAFE_PITCH_LIMIT: f32 = -1.56;
const OBE_SMOOTHING_FACTOR: f32 = 0.01;
const OBE_RISE_PER_STEP: f32 = 6.0;
const OBE_YAW_RANGE: std::ops::Range<f32> = 0.01..0.03;

// `ConfirmOnRed` mode is for debugging. When `BULLET_COLOR_MODE` is in this
// mode, a provisional bullet fired by the local player is white, and turns red
// on promotion (confirmation from server). Similarly, a bullet fired by a
// remote player is spawned as white at the player's interpolated position, and
// turns red after fast-forwarding (blending) towards the extrapolated position
// has finished.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulletColorMode {
    ConfirmThenRed,
    FadeToRed,
}

pub struct Game {
    pub local_player_index: usize,
    pub maze: Maze,
    pub maze_meshes: MazeMeshes,
    pub players: Vec<Player>,
    pub info_map: info::map::MapOverlay,
    pub input_history: Ring<PlayerInput, INPUT_HISTORY_LENGTH>, // 256: ~4.3s at 60Hz.
    pub snapshot_buffer: NetworkBuffer<Snapshot, SNAPSHOT_BUFFER_LENGTH>, // 16 broadcasts, 0.8s at 20Hz.
    pub is_first_snapshot_received: bool,
    pub last_reconciled_tick: Option<u64>,
    pub bullets: Vec<ClientBullet>,
    pub flash: Option<Fade>,
    pub fade_to_black: Option<Fade>,
    pub fade_to_black_finished: bool,
    pub fire_nonce_counter: u32,
    pub last_fire_tick: Option<u64>,
    pub last_sim_tick: u64,
    pub interpolated_positions: Vec<Vec3>,
    pub pending_bullet_events: Vec<BulletEvent>,
    after_game_chat_sent: bool,
    obe_effect: Option<ObeEffect>,
    oriented_sphere_mesh: OrientedSphereMesh,
    remote_shadow_mesh: DiskMesh,
}

impl Game {
    pub fn new(
        local_player_index: usize,
        initial_data: InitialData,
        maze_meshes: MazeMeshes,
        sim_tick: u64,
        info_map: info::map::MapOverlay,
    ) -> Self {
        let players = initial_data.players;
        let interpolated_positions = players.iter().map(|player| player.state.position).collect();

        Self {
            // `snapshot_buffer.head` will be reset when the first snapshot is
            // inserted, but still we need an initial `head` that's within ±2^15
            // ticks of the tick on which the first snapshot was sent so that
            // the first snapshot's 16-bit wire id will be extended to the
            // correct 64-bit storage id.
            snapshot_buffer: NetworkBuffer::new(sim_tick, 0),
            local_player_index,
            maze: initial_data.maze,
            maze_meshes,
            players,
            info_map,
            input_history: Ring::new(),
            is_first_snapshot_received: false,
            last_reconciled_tick: None,
            bullets: Vec::new(),
            flash: None,
            fade_to_black: None,
            fade_to_black_finished: false,
            fire_nonce_counter: 0,
            last_fire_tick: None,
            last_sim_tick: sim_tick,
            interpolated_positions,
            pending_bullet_events: Vec::new(),
            after_game_chat_sent: false,
            obe_effect: None,
            oriented_sphere_mesh: OrientedSphereMesh::new(),
            remote_shadow_mesh: DiskMesh::new(),
        }
    }

    pub fn update_with_network(
        &mut self,
        clock: &mut Clock,
        network: &mut dyn NetworkHandle,
    ) -> Option<ClientState> {
        if self.fade_to_black_finished && !self.after_game_chat_sent {
            self.after_game_chat_sent = true;
            let message = ClientMessage::EnterAfterGameChat;
            let payload =
                encode_to_vec(&message, standard()).expect("failed to encode after-game chat");
            network.send_message(AppChannel::ReliableOrdered, payload);

            return Some(ClientState::AfterGameChat {
                awaiting_initial_roster: true,
                waiting_for_server: false,
            });
        }

        self.receive_game_messages(network);
        if let Some(new_tail) = self.interpolate(clock.estimated_server_time) {
            self.snapshot_buffer.advance_tail(new_tail);
        }
        if !self.pending_bullet_events.is_empty() {
            self.apply_pending_bullet_events();
        }
        self.advance_simulation(clock, network);

        None
    }

    fn advance_simulation(&mut self, clock: &mut Clock, network: &mut dyn NetworkHandle) {
        // A failsafe to prevent `accumulated_time` from growing ever greater
        // if we fall behind.
        const MAX_TICKS_PER_FRAME: u8 = 8;
        let mut ticks_processed = 0;

        let head = self.snapshot_buffer.head;
        if self.reconcile(head) {
            let start_replay = head + 1;
            let end_replay = clock.sim_tick + 1;

            if start_replay <= end_replay {
                self.apply_input_range_inclusive(start_replay, end_replay);
            }
        }

        while clock.accumulated_time >= TICK_SECS && ticks_processed < MAX_TICKS_PER_FRAME {
            let sim_tick = clock.sim_tick;

            if self.players[self.local_player_index].health > 0 {
                let mut input = input::player_input_from_keys(sim_tick);
                self.prepare_fire_input(sim_tick, &mut input);
                self.send_input(network, input, sim_tick);
                self.input_history.insert(sim_tick, input);
                self.apply_input(sim_tick);
            }

            self.last_sim_tick = sim_tick;
            self.update_bullets(sim_tick);
            clock.accumulated_time -= TICK_SECS;
            clock.sim_tick += 1;
            ticks_processed += 1;

            // If at the limit, discard the backlog to stop a spiral.
            if ticks_processed >= MAX_TICKS_PER_FRAME {
                let ticks_to_skip = (clock.accumulated_time / TICK_SECS).floor() as u64;

                if ticks_to_skip > 0 {
                    clock.sim_tick += ticks_to_skip;

                    // Keep the fractional remainder for smoothness.
                    clock.accumulated_time -= ticks_to_skip as f64 * TICK_SECS;

                    println!(
                        "Death spiral: skipped {} ticks to realign clock. Current `sim_tick`: {}",
                        ticks_to_skip, clock.sim_tick
                    );
                }
            }
        }
    }

    // We send the last four inputs for redundancy to mitigate possible loss of
    // messages on the unreliable channel.
    pub fn send_input(
        &mut self,
        network: &mut dyn NetworkHandle,
        input: PlayerInput,
        sim_tick: u64,
    ) {
        let mut tick = sim_tick;
        for i in 0..4 {
            tick = tick.saturating_sub(i);
            let wire_tick: u16 = tick as u16;
            let wire_input = WireItem {
                id: wire_tick,
                data: input,
            };
            let client_message = ClientMessage::Input(wire_input);
            let payload =
                encode_to_vec(&client_message, standard()).expect("failed to encode player input");
            network.send_message(AppChannel::Unreliable, payload);
        }
    }

    pub fn prepare_fire_input(&mut self, sim_tick: u64, input: &mut PlayerInput) {
        if !input.fire {
            return;
        }

        let cooldown_ticks = bullets::cooldown_ticks();
        let can_fire = self
            .last_fire_tick
            .map(|tick| sim_tick.saturating_sub(tick) >= cooldown_ticks)
            .unwrap_or(true);
        let bullets_in_air = self.bullets.iter().filter(|bullet| bullet.is_local).count();

        if !can_fire || bullets_in_air >= bullets::MAX_BULLETS_PER_PLAYER {
            return;
        }

        let local_state = &self.players[self.local_player_index].state;
        let direction = bullets::direction_from_yaw_pitch(local_state.yaw, local_state.pitch);
        if direction == Vec3::ZERO {
            return;
        }

        let fire_nonce = self.fire_nonce_counter;
        self.fire_nonce_counter = self.fire_nonce_counter.wrapping_add(1);
        self.last_fire_tick = Some(sim_tick);
        input.fire_nonce = Some(fire_nonce);

        let position = bullets::spawn_position(local_state.position, direction);
        let velocity = direction * bullets::SPEED;
        self.bullets.push(ClientBullet::new_provisional(
            fire_nonce, position, velocity, sim_tick,
        ));
    }

    // TODO: Consider disparity in naming between snapshot as data without id,
    // and snapshot as WireItem together with id.
    pub fn receive_game_messages(&mut self, network: &mut dyn NetworkHandle) {
        let start_time = Instant::now();
        let mut messages_received: u32 = 0;
        let mut is_shedding_load = false;

        while let Some(data) = network.receive_message(AppChannel::Unreliable) {
            if messages_received % 10 == 0 && start_time.elapsed() > NETWORK_TIME_BUDGET {
                if !is_shedding_load {
                    println!(
                        "Exceeded the time budget. Discarding other snapshots to flush the queue."
                    );
                    is_shedding_load = true;
                }
            }

            if is_shedding_load {
                continue;
            }

            messages_received += 1;

            match decode_from_slice::<ServerMessage, _>(&data, standard()) {
                Ok((ServerMessage::Snapshot(snapshot), _)) => {
                    self.snapshot_buffer.insert(snapshot);
                }
                Ok((ServerMessage::BulletEvent(event), _)) => {
                    self.pending_bullet_events.push(event);
                }
                Ok((other, _)) => {
                    eprintln!(
                        "unexpected message type received from server: {}",
                        other.variant_name()
                    );
                }
                Err(error) => {
                    eprintln!("failed to decode server message: {}", error);
                }
            }
        }

        while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
            match decode_from_slice::<ServerMessage, _>(&data, standard()) {
                Ok((ServerMessage::BulletEvent(event), _)) => {
                    self.pending_bullet_events.push(event);
                }
                Ok((other, _)) => {
                    eprintln!(
                        "unexpected reliable message type received from server: {}",
                        other.variant_name()
                    );
                }
                Err(error) => {
                    eprintln!("failed to decode reliable server message: {}", error);
                }
            }
        }
    }

    pub fn reconcile(&mut self, head: u64) -> bool {
        if let Some(last) = self.last_reconciled_tick {
            if head <= last {
                return false;
            }
        }

        if let Some(snapshot) = self.snapshot_buffer.get(head) {
            self.is_first_snapshot_received = true;
            self.last_reconciled_tick = Some(head);

            let local_state = &mut self.players[self.local_player_index].state;
            local_state.position = snapshot.local.position;
            local_state.velocity = snapshot.local.velocity;
            local_state.yaw = snapshot.local.yaw;
            local_state.pitch = snapshot.local.pitch;
            local_state.yaw_velocity = snapshot.local.yaw_velocity;
            local_state.pitch_velocity = snapshot.local.pitch_velocity;

            true
        } else {
            false
        }
    }

    pub fn apply_input_range_inclusive(&mut self, from: u64, to: u64) {
        for tick in from..=to {
            self.apply_input(tick);
        }
    }

    pub fn apply_input(&mut self, tick: u64) {
        let own_index = self.local_player_index;

        // Needed to detect collisions with other players.
        let player_positions: Vec<(usize, Vec3)> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.health > 0)
            .map(|(i, p)| (i, p.state.position))
            .collect();

        if let Some(input) = self.input_history.get(tick) {
            self.players[own_index].state.update(
                &self.maze,
                input,
                own_index,
                &player_positions,
                0.5,
            );
        }
    }

    pub fn interpolate(&mut self, estimated_server_time: f64) -> Option<u64> {
        let interpolation_time = estimated_server_time - INTERPOLATION_DELAY_SECS;
        let start_search_tick = crate::time::tick_from_time(interpolation_time);
        let mut tick_a = start_search_tick;
        let limit = 8;

        while self.snapshot_buffer.get(tick_a).is_none() {
            if start_search_tick - tick_a > limit {
                return None;
            };
            tick_a -= 1;
        }

        let mut tick_b = start_search_tick + 1;

        while self.snapshot_buffer.get(tick_b).is_none() {
            if tick_b - (start_search_tick + 1) > limit {
                return None;
            }
            tick_b += 1;
        }

        let snapshot_a = self.snapshot_buffer.get(tick_a)?;
        let snapshot_b = self.snapshot_buffer.get(tick_b)?;

        let time_a = crate::time::time_from_tick(tick_a);
        let time_b = crate::time::time_from_tick(tick_b);
        let alpha = (interpolation_time - time_a) / (time_b - time_a);
        let alpha = alpha as f32;

        let remote_a = &snapshot_a.remote;
        let remote_b = &snapshot_b.remote;
        let mut remote_index = 0;

        for (index, player) in self.players.iter_mut().enumerate() {
            if index == self.local_player_index {
                continue;
            }

            let Some(a) = remote_a.get(remote_index) else {
                return None;
            };
            let Some(b) = remote_b.get(remote_index) else {
                return None;
            };

            let state = &mut player.state;
            state.position = a.position + (b.position - a.position) * alpha;
            state.yaw = a.yaw + (b.yaw - a.yaw) * alpha;
            state.pitch = a.pitch + (b.pitch - a.pitch) * alpha;

            remote_index += 1;
        }

        self.interpolated_positions.clear();
        self.interpolated_positions
            .extend(self.players.iter().map(|player| player.state.position));

        // We subtract a big safety margin in case `estimated_server_time` goes
        // momentarily backwards due to network instability.
        Some(tick_a - 60)
    }

    pub fn apply_pending_bullet_events(&mut self) {
        let events = std::mem::take(&mut self.pending_bullet_events);
        for event in events {
            self.apply_bullet_event(event);
        }
    }

    // TODO: `prediction_alpha` would be for smoothing the local player between
    // ticks if I allow faster than 60Hz frame rate for devices that support it.
    pub fn draw(&mut self, _prediction_alpha: f64, assets: &Assets, fps: &FrameRate) {
        clear_background(BEIGE);

        let i = self.local_player_index;
        let mut position = self.players[i].state.position;
        let mut yaw = self.players[i].state.yaw;
        let mut pitch = self.players[i].state.pitch;

        if let Some(obe_effect) = &mut self.obe_effect {
            obe_effect.update();
            position.y += obe_effect.height_offset;
            yaw += obe_effect.yaw_offset;
            pitch = obe_effect.pitch;
        }

        set_camera(&Camera3D {
            position,
            target: position
                + vec3(
                    -yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                ),
            up: vec3(0.0, 1.0, 0.0),
            z_near: 0.1,
            z_far: 5000.0,
            ..Default::default()
        });

        self.maze.draw(&self.maze_meshes);
        self.draw_local_player_shadow();
        self.draw_remote_players(assets);
        self.draw_bullets();
        info::draw(self, assets, fps, info::INFO_SCALE);

        // Handle fading to black when the local player dies. This block must
        // be placed after drawing the scene so that the fade covers everything
        // and not just the background. If this becomes a problem, consider
        // decoupling the drawing of the fade (and likewise the flash) from
        // checking whether it's still fading.
        if let Some(fade) = &self.fade_to_black {
            if !fade.is_still_fading_so_draw() {
                clear_background(BLACK); // Avoids a brief flash after the fade completes.
                self.fade_to_black_finished = true;
            }
        }

        // Draw fading flash over the whole screen to indicate that the local
        // player has recently been hit.
        if let Some(flash) = &self.flash {
            if !flash.is_still_fading_so_draw() {
                self.flash = None;
            }
        }
    }

    fn draw_remote_players(&mut self, assets: &Assets) {
        for index in 0..self.players.len() {
            if index == self.local_player_index {
                continue;
            }
            if !self.players[index].alive {
                continue;
            }

            let position = self.players[index].state.position;
            let yaw = self.players[index].state.yaw;
            let pitch = self.players[index].state.pitch;
            self.draw_player_shadow(position);

            self.oriented_sphere_mesh.draw(
                position,
                player::RADIUS,
                Some(&assets.ball_texture),
                WHITE,
                yaw,
                pitch,
            );
        }
    }

    fn draw_local_player_shadow(&mut self) {
        let local_player = &self.players[self.local_player_index];
        if !local_player.alive {
            return;
        }

        self.draw_player_shadow(local_player.state.position);
    }

    fn draw_player_shadow(&mut self, position: Vec3) {
        let shadow_color = Color::new(0.2, 0.2, 0.2, 0.35);
        let shadow_radius = player::RADIUS * 0.9;
        const SHADOW_HEIGHT: f32 = 0.12;

        let shadow_position = vec3(position.x, SHADOW_HEIGHT, position.z);
        self.remote_shadow_mesh
            .draw(shadow_position, shadow_radius, shadow_color);
    }

    fn draw_bullets(&self) {
        const BULLET_DRAW_RADIUS: f32 = 4.0;
        let draw_offset = (BULLET_DRAW_RADIUS - bullets::BULLET_RADIUS).max(0.0);

        for bullet in &self.bullets {
            let color = if bullet.blend_ticks_left > 0 {
                WHITE
            } else {
                match BULLET_COLOR_MODE {
                    BulletColorMode::ConfirmThenRed => {
                        if bullet.confirmed {
                            RED
                        } else {
                            WHITE
                        }
                    }
                    BulletColorMode::FadeToRed => {
                        let fade = bullet.fade_amount(self.last_sim_tick);
                        Color::new(1.0, fade, fade, fade)
                    }
                }
            };
            // Keep the visual sphere aligned with the physics radius.
            let draw_position = bullet.position + vec3(0.0, draw_offset, 0.0);
            draw_sphere(draw_position, BULLET_DRAW_RADIUS, None, color);
        }
    }

    fn update_bullets(&mut self, sim_tick: u64) {
        const PROVISIONAL_TIMEOUT_TICKS: u64 = 30;
        let lifespan_ticks = (bullets::LIFESPAN_SECS / TICK_SECS).ceil() as u64;
        let maze = &self.maze;

        self.bullets.retain_mut(|bullet| {
            if sim_tick > bullet.last_update_tick {
                let ticks = sim_tick - bullet.last_update_tick;
                if bullet.confirmed {
                    for _ in 0..ticks {
                        bullet.advance(1);
                        bullet.bounce_off_ground();
                        match bullet.bounce_off_wall(maze) {
                            bullets::WallBounce::Stuck => {}
                            bullets::WallBounce::Bounce => {}
                            bullets::WallBounce::None => {}
                        }
                    }
                    bullet.apply_blend(ticks);
                } else {
                    for _ in 0..ticks {
                        bullet.advance(1);
                        bullet.bounce_off_ground();
                        match bullet.bounce_off_wall(maze) {
                            bullets::WallBounce::Stuck => return false,
                            bullets::WallBounce::Bounce => {}
                            bullets::WallBounce::None => {}
                        }

                        if bullet.has_bounced_enough() {
                            return false;
                        }
                    }
                }
                bullet.last_update_tick = sim_tick;
            }

            if !bullet.confirmed {
                if sim_tick.saturating_sub(bullet.spawn_tick) > PROVISIONAL_TIMEOUT_TICKS {
                    return false;
                }
                if sim_tick.saturating_sub(bullet.spawn_tick) > lifespan_ticks {
                    return false;
                }
            }

            true
        });
    }

    fn apply_bullet_event(&mut self, event: BulletEvent) {
        match event {
            BulletEvent::Spawn {
                bullet_id,
                tick,
                position,
                velocity,
                fire_nonce,
                shooter_index,
            } => {
                let adjusted_position =
                    extrapolate_bullet_position(position, velocity, tick, self.last_sim_tick);
                const PROMOTION_BLEND_TICKS: u8 = 4;
                const REMOTE_SPAWN_BLEND_TICKS: u8 = 4;

                if shooter_index == self.local_player_index {
                    if let Some(fire_nonce) = fire_nonce {
                        if let Some(bullet) = self
                            .bullets
                            .iter_mut()
                            .find(|bullet| bullet.is_provisional_for(fire_nonce))
                        {
                            bullet.confirm(bullet_id, velocity, self.last_sim_tick);
                            bullet.start_blend(adjusted_position, PROMOTION_BLEND_TICKS);
                            return;
                        }

                        self.bullets.push(ClientBullet::new_confirmed_local(
                            bullet_id,
                            adjusted_position,
                            velocity,
                            self.last_sim_tick,
                        ));
                        return;
                    }
                }

                self.bullets.push(ClientBullet::new_confirmed(
                    bullet_id,
                    adjusted_position,
                    velocity,
                    self.last_sim_tick,
                ));

                if shooter_index != self.local_player_index {
                    if let Some(owner_position) = self.interpolated_positions.get(shooter_index) {
                        if let Some(bullet) = self.bullets.last_mut() {
                            bullet.position = *owner_position;
                            bullet.start_blend(adjusted_position, REMOTE_SPAWN_BLEND_TICKS);
                        }
                    }
                }
            }
            BulletEvent::HitInanimate {
                bullet_id,
                tick,
                position,
                velocity,
            } => {
                if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == Some(bullet_id)) {
                    bullet.position = position;
                    bullet.velocity = velocity;
                    bullet.last_update_tick = tick;
                }
            }
            BulletEvent::HitPlayer {
                bullet_id,
                tick,
                position,
                velocity,
                target_index,
                target_health,
            } => {
                if let Some(player) = self.players.get_mut(target_index) {
                    player.health = target_health;
                    player.alive = target_health > 0;
                }

                if target_index == self.local_player_index {
                    if target_health == 0 {
                        self.fade_to_black = Some(fade::new_fade_to_black());
                        self.fade_to_black_finished = false;
                        if self.obe_effect.is_none() {
                            self.obe_effect =
                                Some(ObeEffect::new(self.players[target_index].state));
                        }
                    } else {
                        self.flash = Some(fade::new_flash());
                    }
                }

                if target_health == 0 {
                    self.bullets.retain(|b| b.id != Some(bullet_id));
                } else if let Some(bullet) =
                    self.bullets.iter_mut().find(|b| b.id == Some(bullet_id))
                {
                    bullet.position = position;
                    bullet.velocity = velocity;
                    bullet.last_update_tick = tick;
                }
            }
            BulletEvent::Expire { bullet_id, .. } => {
                self.bullets.retain(|b| b.id != Some(bullet_id));
            }
        }
    }
}

struct ObeEffect {
    accumulator: f32,
    yaw_increment: f32,
    yaw_offset: f32,
    pitch: f32,
    height_offset: f32,
}

impl ObeEffect {
    fn new(local_state: player::PlayerState) -> Self {
        let mut rng = rng();
        let mut yaw_increment = rng.random_range(OBE_YAW_RANGE);
        if rng.random_range(0..2) == 0 {
            yaw_increment = -yaw_increment;
        }

        Self {
            accumulator: 0.0,
            yaw_increment,
            yaw_offset: 0.0,
            pitch: local_state.pitch,
            height_offset: 0.0,
        }
    }

    fn update(&mut self) {
        self.accumulator += get_frame_time();

        while self.accumulator >= OBE_TIME_STEP {
            self.height_offset += OBE_RISE_PER_STEP;
            self.yaw_offset += self.yaw_increment;
            self.pitch += (OBE_SAFE_PITCH_LIMIT - self.pitch) * OBE_SMOOTHING_FACTOR;
            self.accumulator -= OBE_TIME_STEP;
        }
    }
}

struct OrientedSphereMesh {
    base_vertices: Vec<(Vec3, Vec2)>,
    mesh: Mesh,
}

struct DiskMesh {
    base_vertices: Vec<(Vec3, Vec2)>,
    mesh: Mesh,
}

impl DiskMesh {
    fn new() -> Self {
        const SLICES: usize = 24;

        let triangle_count = SLICES;
        let mut base_vertices = Vec::with_capacity(triangle_count * 3);

        use std::f32::consts::PI;
        let two_pi = PI * 2.0;

        for i in 0..SLICES {
            let angle_a = (i as f32) * two_pi / (SLICES as f32);
            let angle_b = ((i + 1) as f32) * two_pi / (SLICES as f32);

            let v1 = vec3(angle_a.cos(), 0.0, angle_a.sin());
            let v2 = vec3(angle_b.cos(), 0.0, angle_b.sin());

            base_vertices.push((Vec3::ZERO, vec2(0.5, 0.5)));
            base_vertices.push((
                v1,
                vec2(0.5 + 0.5 * angle_a.cos(), 0.5 + 0.5 * angle_a.sin()),
            ));
            base_vertices.push((
                v2,
                vec2(0.5 + 0.5 * angle_b.cos(), 0.5 + 0.5 * angle_b.sin()),
            ));
        }

        let vertices = base_vertices
            .iter()
            .map(|(position, uv)| Vertex::new2(*position, *uv, WHITE))
            .collect();
        let indices = (0..base_vertices.len() as u16).collect();

        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };

        Self {
            base_vertices,
            mesh,
        }
    }

    fn draw(&mut self, center: Vec3, radius: f32, color: Color) {
        let scale = vec3(radius, 1.0, radius);
        let color_bytes: [u8; 4] = color.into();

        for (vertex, (base_position, uv)) in
            self.mesh.vertices.iter_mut().zip(self.base_vertices.iter())
        {
            vertex.position = *base_position * scale + center;
            vertex.uv = *uv;
            vertex.color = color_bytes;
        }

        self.mesh.texture = None;
        draw_mesh(&self.mesh);
    }
}

impl OrientedSphereMesh {
    fn new() -> Self {
        const RINGS: usize = 16;
        const SLICES: usize = 16;

        let triangle_count = (RINGS + 1) * SLICES * 2;
        let mut base_vertices = Vec::with_capacity(triangle_count * 3);

        let mut push_triangle = |v1: Vec3, uv1: Vec2, v2: Vec3, uv2: Vec2, v3: Vec3, uv3: Vec2| {
            base_vertices.push((v1, uv1));
            base_vertices.push((v2, uv2));
            base_vertices.push((v3, uv3));
        };

        use std::f32::consts::PI;
        let pi34 = PI / 2. * 3.;
        let pi2 = PI * 2.;
        let rings = RINGS as f32;
        let slices = SLICES as f32;

        for i in 0..RINGS + 1 {
            for j in 0..SLICES {
                let i = i as f32;
                let j = j as f32;

                let v1 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).cos(),
                );
                let uv1 = vec2(i / rings, j / slices);
                let v2 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv2 = vec2((i + 1.) / rings, (j + 1.) / slices);
                let v3 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * (j * pi2 / slices).cos(),
                );
                let uv3 = vec2((i + 1.) / rings, j / slices);
                push_triangle(v1, uv1, v2, uv2, v3, uv3);

                let v1 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * (j * pi2 / slices).cos(),
                );
                let uv1 = vec2(i / rings, j / slices);
                let v2 = vec3(
                    (pi34 + (PI / (rings + 1.)) * i).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).sin(),
                    (pi34 + (PI / (rings + 1.)) * i).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv2 = vec2(i / rings, (j + 1.) / slices);
                let v3 = vec3(
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).sin(),
                    (pi34 + (PI / (rings + 1.)) * (i + 1.)).cos() * ((j + 1.) * pi2 / slices).cos(),
                );
                let uv3 = vec2((i + 1.) / rings, (j + 1.) / slices);
                push_triangle(v1, uv1, v2, uv2, v3, uv3);
            }
        }

        let vertices = base_vertices
            .iter()
            .map(|(position, uv)| Vertex::new2(*position, *uv, WHITE))
            .collect();
        let indices = (0..base_vertices.len() as u16).collect();

        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };

        Self {
            base_vertices,
            mesh,
        }
    }

    fn draw(
        &mut self,
        center: Vec3,
        radius: f32,
        texture: Option<&Texture2D>,
        color: Color,
        yaw: f32,
        pitch: f32,
    ) {
        let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        let scale = vec3(radius, radius, radius);
        let color_bytes: [u8; 4] = color.into();

        for (vertex, (base_position, uv)) in
            self.mesh.vertices.iter_mut().zip(self.base_vertices.iter())
        {
            vertex.position = rotation.mul_vec3(*base_position * scale) + center;
            vertex.uv = *uv;
            vertex.color = color_bytes;
        }

        self.mesh.texture = texture.cloned();
        draw_mesh(&self.mesh);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClientBullet {
    pub id: Option<u32>,
    pub fire_nonce: Option<u32>,
    pub position: Vec3,
    pub velocity: Vec3,
    pub last_update_tick: u64,
    pub spawn_tick: u64,
    pub confirmed: bool,
    pub is_local: bool,
    pub bounces: u8,
    pub blend_step: Vec3,
    pub blend_ticks_left: u8,
}

impl ClientBullet {
    pub fn new_confirmed(id: u32, position: Vec3, velocity: Vec3, last_update_tick: u64) -> Self {
        Self {
            id: Some(id),
            fire_nonce: None,
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: true,
            is_local: false,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn new_confirmed_local(
        id: u32,
        position: Vec3,
        velocity: Vec3,
        last_update_tick: u64,
    ) -> Self {
        Self {
            id: Some(id),
            fire_nonce: None,
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: true,
            is_local: true,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn new_provisional(
        fire_nonce: u32,
        position: Vec3,
        velocity: Vec3,
        last_update_tick: u64,
    ) -> Self {
        Self {
            id: None,
            fire_nonce: Some(fire_nonce),
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: false,
            is_local: true,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn confirm(&mut self, id: u32, velocity: Vec3, tick: u64) {
        self.id = Some(id);
        self.fire_nonce = None;
        self.velocity = velocity;
        self.last_update_tick = tick;
        self.confirmed = true;
        self.is_local = true;
        self.bounces = 0;
        self.blend_step = Vec3::ZERO;
        self.blend_ticks_left = 0;
    }

    pub fn advance(&mut self, ticks: u64) {
        let delta = ticks as f32 * common::constants::TICK_SECS_F32;
        self.position += self.velocity * delta;
    }

    pub fn is_provisional_for(&self, fire_nonce: u32) -> bool {
        !self.confirmed && self.fire_nonce == Some(fire_nonce)
    }

    pub fn fade_amount(&self, sim_tick: u64) -> f32 {
        let age = sim_tick.saturating_sub(self.spawn_tick);
        let lifespan_ticks = (bullets::LIFESPAN_SECS / TICK_SECS).ceil() as u64;
        if lifespan_ticks == 0 {
            return 1.0;
        }
        let t = age as f32 / lifespan_ticks as f32;
        (1.0 - t).clamp(0.0, 1.0)
    }

    pub fn start_blend(&mut self, target: Vec3, ticks: u8) {
        if ticks == 0 {
            self.position = target;
            self.blend_step = Vec3::ZERO;
            self.blend_ticks_left = 0;
            return;
        }

        self.blend_step = (target - self.position) / ticks as f32;
        self.blend_ticks_left = ticks;
    }

    // TODO: Push `if` up?
    pub fn apply_blend(&mut self, ticks: u64) {
        if self.blend_ticks_left == 0 {
            return;
        }

        let steps = ticks.min(self.blend_ticks_left as u64) as u8;
        self.position += self.blend_step * steps as f32;
        self.blend_ticks_left -= steps;
    }

    pub fn has_bounced_enough(&self) -> bool {
        self.bounces > bullets::MAX_BOUNCES
    }

    pub fn bounce_off_ground(&mut self) -> bool {
        bullets::bounce_off_ground(&mut self.position, &mut self.velocity, &mut self.bounces)
    }

    pub fn bounce_off_wall(&mut self, maze: &Maze) -> bullets::WallBounce {
        bullets::bounce_off_wall(&self.position, &mut self.velocity, &mut self.bounces, maze)
    }
}

fn extrapolate_bullet_position(
    position: Vec3,
    velocity: Vec3,
    event_tick: u64,
    sim_tick: u64,
) -> Vec3 {
    if sim_tick <= event_tick {
        return position;
    }

    let ticks = sim_tick - event_tick;
    position + velocity * (ticks as f32 * TICK_SECS_F32)
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
            .field("local_player_index", &self.local_player_index)
            .field("maze", &self.maze)
            .field("maze_meshes", &self.maze_meshes)
            .field("players", &self.players)
            .field("input_history", &self.input_history)
            .finish()
    }
}
