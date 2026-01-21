use std::{
    fmt,
    time::{Duration, Instant},
};

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use macroquad::{
    audio::{PlaySoundParams, play_sound, play_sound_once},
    prelude::*,
};

use crate::{
    after_game_chat::AfterGameChat,
    assets::Assets,
    fade::{self, Fade},
    frame::FrameRate,
    game::input,
    game::{
        obe::ObeEffect,
        world::{
            avatar::{DiskMesh, OrientedSphereMesh},
            bullet::{self, BULLET_COLOR_MODE, BulletColorMode, ClientBullet},
            maze::{MazeExtension, MazeMeshes},
            sky::Sky,
        },
    },
    info,
    net::NetworkHandle,
    session::Clock,
    state::ClientState,
    time::INTERPOLATION_DELAY_SECS,
};
use common::{
    bullets::{self, BULLET_SHELL_RADIUS},
    constants::{INPUT_HISTORY_LENGTH, SNAPSHOT_BUFFER_LENGTH, TICK_SECS},
    maze::{self, CELL_SIZE, Maze},
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
const REPULSION_STRENGTH: f32 = 0.5; // For collisions.
const NORMAL_FOV: f32 = 45.0_f32.to_radians();
const ZOOMED_FOV: f32 = 10.0_f32.to_radians();

pub struct Game {
    pub local_player_index: usize,
    pub players: Vec<Player>,
    pub info_map: info::map::MapOverlay,
    pub maze: Maze,
    maze_meshes: MazeMeshes,
    sky: Sky,
    input_history: Ring<PlayerInput, INPUT_HISTORY_LENGTH>, // 256: ~4.3s at 60Hz.
    snapshot_buffer: NetworkBuffer<Snapshot, SNAPSHOT_BUFFER_LENGTH>, // 16 broadcasts, 0.8s at 20Hz.
    is_first_snapshot_received: bool,
    last_reconciled_tick: Option<u64>,
    bullets: Vec<ClientBullet>,
    flash: Option<Fade>,
    fade_to_black: Option<Fade>,
    fade_to_black_finished: bool,
    fire_nonce_counter: u32,
    last_fire_tick: Option<u64>,
    last_sim_tick: u64,
    pending_bullet_events: Vec<BulletEvent>,
    after_game_chat_sent: bool,
    obe_effect: Option<ObeEffect>,
    player_avatar_mesh: OrientedSphereMesh,
    player_shadow_mesh: DiskMesh,
    previous_local_state: StaticState,
    fov: f32,
}

impl Game {
    pub fn new(
        local_player_index: usize,
        initial_data: InitialData,
        maze_meshes: MazeMeshes,
        sky_mesh: Mesh,
        sim_tick: u64,
        info_map: info::map::MapOverlay,
    ) -> Self {
        let sky = Sky { mesh: sky_mesh };
        let players = initial_data.players;
        let previous_local_state = StaticState::new(&players[local_player_index]);

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
            sky,
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
            pending_bullet_events: Vec::new(),
            after_game_chat_sent: false,
            obe_effect: None,
            player_avatar_mesh: OrientedSphereMesh::new(),
            player_shadow_mesh: DiskMesh::new(),
            previous_local_state,
            fov: NORMAL_FOV,
        }
    }

    pub fn update_with_network(
        &mut self,
        clock: &mut Clock,
        network: &mut dyn NetworkHandle,
        assets: &Assets,
    ) -> Option<ClientState> {
        if self.fade_to_black_finished && !self.after_game_chat_sent {
            self.after_game_chat_sent = true;
            let message = ClientMessage::EnterAfterGameChat;
            let payload =
                encode_to_vec(&message, standard()).expect("failed to encode after-game chat");
            network.send_message(AppChannel::ReliableOrdered, payload);

            return Some(ClientState::AfterGameChat(AfterGameChat {
                awaiting_initial_roster: true,
                waiting_for_server: false,
            }));
        }

        self.receive_game_messages(network);
        if let Some(new_tail) = self.interpolate(clock.estimated_server_time) {
            self.snapshot_buffer.advance_tail(new_tail);
        }
        if !self.pending_bullet_events.is_empty() {
            self.apply_pending_bullet_events(assets);
        }
        self.advance_simulation(clock, network, assets);

        None
    }

    fn advance_simulation(
        &mut self,
        clock: &mut Clock,
        network: &mut dyn NetworkHandle,
        assets: &Assets,
    ) {
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
                self.prepare_fire_input(sim_tick, &mut input, assets);
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

    pub fn prepare_fire_input(&mut self, sim_tick: u64, input: &mut PlayerInput, assets: &Assets) {
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
        play_sound_once(&assets.gun_sound);
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
                Ok((ServerMessage::UserLeft { username }, _)) => {
                    if let Some(player) = self.players.iter_mut().find(|p| p.name == username) {
                        player.disconnected = true;
                    }
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
                Ok((ServerMessage::UserLeft { username }, _)) => {
                    if let Some(player) = self.players.iter_mut().find(|p| p.name == username) {
                        player.disconnected = true;
                    }
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

            let local_player = &mut self.players[self.local_player_index];
            self.previous_local_state = StaticState::new(&local_player);

            let local_state = &mut local_player.state;

            local_state.position = snapshot.local.position;
            local_state.velocity = snapshot.local.velocity;
            local_state.yaw = snapshot.local.yaw;
            local_state.pitch = snapshot.local.pitch;
            local_state.yaw_velocity = snapshot.local.yaw_velocity;
            local_state.pitch_velocity = snapshot.local.pitch_velocity;
            local_state.is_zoomed = snapshot.local.is_zoomed;

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

        // This is needed to detect collisions with other players.
        let player_positions: Vec<(usize, Vec3)> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_alive())
            .map(|(i, p)| (i, p.state.position))
            .collect();

        if let Some(input) = self.input_history.get(tick) {
            let local_player = &mut self.players[own_index];

            self.previous_local_state = StaticState::new(&local_player);

            local_player.state.update(
                &self.maze,
                input,
                own_index,
                &player_positions,
                REPULSION_STRENGTH,
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

        // The returned value will become the new `tail` of the
        // `snapshot_buffer`. We subtract a big safety margin in case
        // `estimated_server_time` goes momentarily backwards due to network
        // instability.
        Some(tick_a - 60)
    }

    pub fn apply_pending_bullet_events(&mut self, assets: &Assets) {
        let events = std::mem::take(&mut self.pending_bullet_events);
        for event in events {
            self.apply_bullet_event(event, assets);
        }
    }

    pub fn draw(&mut self, tick_fraction: f32, assets: &Assets, fps: &FrameRate) {
        clear_background(BEIGE);
        self.set_camera(tick_fraction);

        self.sky.draw();
        self.maze.draw(&self.maze_meshes);
        self.draw_players(assets);
        self.draw_bullets(tick_fraction);
        info::draw(self, assets, fps);

        // This function must be called after drawing the scene so that the fade
        // covers everything and not just the background. If this becomes a
        // problem, consider decoupling the drawing of the fade (and likewise
        // the flash) from checking whether it's still fading.
        self.draw_flash_and_fade();
    }

    fn set_camera(&mut self, tick_fraction: f32) {
        let i = self.local_player_index;
        let local_player_state = self.players[i].state;
        let prev_state = &self.previous_local_state;
        let curr_state = &local_player_state;

        let mut position = prev_state.position.lerp(curr_state.position, tick_fraction);
        let mut yaw = prev_state.yaw + (curr_state.yaw - prev_state.yaw) * tick_fraction;
        let mut pitch = prev_state.pitch + (curr_state.pitch - prev_state.pitch) * tick_fraction;

        if let Some(obe_effect) = &mut self.obe_effect {
            obe_effect.update();

            let [interp_height_offset, interp_yaw_offset, interp_pitch] = obe_effect.interpolate();

            position.y += interp_height_offset;
            yaw += interp_yaw_offset;
            pitch = interp_pitch;
        }

        let target_fov = if local_player_state.is_zoomed {
            ZOOMED_FOV
        } else {
            NORMAL_FOV
        };
        self.fov += (target_fov - self.fov) * 0.1;

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
            z_far: 10000.0,
            fovy: self.fov,
            ..Default::default()
        });
    }

    fn draw_players(&mut self, assets: &Assets) {
        for index in 0..self.players.len() {
            if !self.players[index].is_alive() {
                continue;
            }

            let position = self.players[index].state.position;
            self.draw_player_shadow(position);

            if index == self.local_player_index {
                continue;
            }

            let yaw = self.players[index].state.yaw;
            let pitch = self.players[index].state.pitch;

            self.player_avatar_mesh.draw(
                position,
                player::RADIUS,
                Some(&assets.ball_texture),
                WHITE,
                yaw,
                pitch,
            );
        }
    }

    fn draw_player_shadow(&mut self, position: Vec3) {
        let shadow_color = Color::new(0.2, 0.2, 0.2, 0.35);
        let shadow_radius = player::RADIUS * 0.9;
        let shadow_height: f32 = 0.12;
        let shadow_position = vec3(position.x, shadow_height, position.z);
        self.player_shadow_mesh
            .draw(shadow_position, shadow_radius, shadow_color);
    }

    fn draw_flash_and_fade(&mut self) {
        // Handle fading to black when the local player dies.
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

    fn draw_bullets(&self, tick_fraction: f32) {
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

            let smoothed_position = bullet
                .previous_position
                .lerp(bullet.position, tick_fraction);

            draw_sphere(smoothed_position, BULLET_SHELL_RADIUS, None, color);
        }
    }

    fn update_bullets(&mut self, sim_tick: u64) {
        const PROVISIONAL_TIMEOUT_TICKS: u64 = 30;
        let lifespan_ticks = (bullets::LIFESPAN_SECS / TICK_SECS).ceil() as u64;
        let maze = &self.maze;

        self.bullets.retain_mut(|bullet| {
            if sim_tick > bullet.last_update_tick {
                bullet.previous_position = bullet.position;
                let ticks = sim_tick - bullet.last_update_tick;
                if bullet.confirmed {
                    for _ in 0..ticks {
                        bullet.advance(1);
                        bullets::bounce_off_ground(
                            &mut bullet.position,
                            &mut bullet.velocity,
                            &mut bullet.bounces,
                        );
                        match bullets::bounce_off_wall(
                            &bullet.position,
                            &mut bullet.velocity,
                            &mut bullet.bounces,
                            maze,
                        ) {
                            bullets::WallBounce::Stuck => {}
                            bullets::WallBounce::Bounce => {}
                            bullets::WallBounce::None => {}
                        }
                    }
                    bullet.apply_blend(ticks);
                } else {
                    for _ in 0..ticks {
                        bullet.advance(1);
                        bullets::bounce_off_ground(
                            &mut bullet.position,
                            &mut bullet.velocity,
                            &mut bullet.bounces,
                        );
                        match bullets::bounce_off_wall(
                            &bullet.position,
                            &mut bullet.velocity,
                            &mut bullet.bounces,
                            maze,
                        ) {
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

    fn apply_bullet_event(&mut self, event: BulletEvent, assets: &Assets) {
        match event {
            BulletEvent::Spawn {
                bullet_id,
                tick,
                position,
                velocity,
                fire_nonce,
                shooter_index,
            } => self.handle_bullet_spawn_event(
                bullet_id,
                tick,
                position,
                velocity,
                fire_nonce,
                shooter_index,
                assets,
            ),
            BulletEvent::HitInanimate {
                bullet_id,
                tick,
                position,
                velocity,
            } => self.handle_bullet_hit_inanimate_event(bullet_id, tick, position, velocity),
            BulletEvent::HitPlayer {
                bullet_id,
                tick,
                position,
                velocity,
                target_index,
                target_health,
            } => self.handle_bullet_hit_player_event(
                bullet_id,
                tick,
                position,
                velocity,
                target_index,
                target_health,
                assets,
            ),
            BulletEvent::Expire { bullet_id, .. } => {
                self.handle_bullet_expire_event(bullet_id);
            }
        }
    }

    fn handle_bullet_spawn_event(
        &mut self,
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
        fire_nonce: Option<u32>,
        shooter_index: usize,
        assets: &Assets,
    ) {
        let adjusted_position =
            bullet::extrapolate_position(position, velocity, tick, self.last_sim_tick);

        if shooter_index == self.local_player_index {
            self.handle_local_bullet_spawn(bullet_id, adjusted_position, velocity, fire_nonce);
        } else {
            self.handle_remote_bullet_spawn(
                bullet_id,
                adjusted_position,
                velocity,
                shooter_index,
                assets,
            );
        }
    }

    fn handle_local_bullet_spawn(
        &mut self,
        bullet_id: u32,
        adjusted_position: Vec3,
        velocity: Vec3,
        fire_nonce: Option<u32>,
    ) {
        const PROMOTION_BLEND_TICKS: u8 = 4;

        let fire_nonce = fire_nonce.expect(
            "local player bullet spawn event must include fire_nonce; \
             this indicates a protocol violation or server bug",
        );

        // If we can find a provisional bullet with matching nonce, start
        // blending from its position, otherwise just spawn the new bullet at
        // the position indicated by the server.
        if let Some(provisional) = self
            .bullets
            .iter_mut()
            .find(|bullet| bullet.is_provisional_for(fire_nonce))
        {
            provisional.confirm(bullet_id, velocity, self.last_sim_tick);
            provisional.start_blend(adjusted_position, PROMOTION_BLEND_TICKS);
        } else {
            self.spawn_confirmed_local_bullet(bullet_id, adjusted_position, velocity);
        }
    }

    fn spawn_confirmed_local_bullet(&mut self, bullet_id: u32, position: Vec3, velocity: Vec3) {
        self.bullets.push(ClientBullet::new_confirmed_local(
            bullet_id,
            position,
            velocity,
            self.last_sim_tick,
        ));
    }

    fn handle_remote_bullet_spawn(
        &mut self,
        bullet_id: u32,
        adjusted_position: Vec3,
        velocity: Vec3,
        shooter_index: usize,
        assets: &Assets,
    ) {
        const REMOTE_SPAWN_BLEND_TICKS: u8 = 4;

        let shooter_position = self.players[shooter_index].state.position;

        self.bullets.push(ClientBullet::new_confirmed(
            bullet_id,
            shooter_position,
            velocity,
            self.last_sim_tick,
        ));

        if self.line_of_sight(shooter_index) {
            self.play_remote_gunshot_sound(shooter_index, assets);
        }

        if let Some(bullet) = self.bullets.last_mut() {
            bullet.start_blend(adjusted_position, REMOTE_SPAWN_BLEND_TICKS);
        }
    }

    fn play_remote_gunshot_sound(&self, shooter_index: usize, assets: &Assets) {
        let distance = self.players[self.local_player_index]
            .state
            .position
            .distance(self.players[shooter_index].state.position);
        let volume = distance / maze::RADIUS as f32 * CELL_SIZE * 2.0;
        play_sound(
            &assets.gun_sound,
            PlaySoundParams {
                looped: false,
                volume,
                ..Default::default()
            },
        );
    }

    fn line_of_sight(&self, remote_player_index: usize) -> bool {
        let local_pos = self.players[self.local_player_index].state.position;
        let remote_pos = self.players[remote_player_index].state.position;

        let maze = &self.maze;
        let (local_grid_x, local_grid_z) = maze
            .grid_coordinates_from_position(&local_pos)
            .expect("local player should be in grid");
        let (remote_grid_x, remote_grid_z) = maze
            .grid_coordinates_from_position(&remote_pos)
            .expect("remote player should be in grid");

        if (local_grid_x as i32 - remote_grid_x as i32).abs() == 1
            && (local_grid_z as i32 - remote_grid_z as i32).abs() == 1
        {
            return false;
        }

        if local_grid_x != remote_grid_x && local_grid_z != remote_grid_z {
            return false;
        }

        if local_grid_x == remote_grid_x {
            let min_z = local_grid_z.min(remote_grid_z);
            let max_z = local_grid_z.max(remote_grid_z);

            for z in min_z..=max_z {
                if maze.grid[z as usize][local_grid_x as usize] == 1 {
                    return false;
                }
            }
        } else {
            let min_x = local_grid_x.min(remote_grid_x);
            let max_x = local_grid_x.max(remote_grid_x);

            for x in min_x..=max_x {
                if maze.grid[local_grid_z as usize][x as usize] == 1 {
                    return false;
                }
            }
        }

        true
    }

    fn handle_bullet_hit_inanimate_event(
        &mut self,
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
    ) {
        if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == Some(bullet_id)) {
            bullet.position = position;
            bullet.velocity = velocity;
            bullet.last_update_tick = tick;
            bullet.blend_ticks_left = 0;
            bullet.blend_step = Vec3::ZERO;
        }
    }

    fn handle_bullet_hit_player_event(
        &mut self,
        bullet_id: u32,
        tick: u64,
        position: Vec3,
        velocity: Vec3,
        target_index: usize,
        target_health: u8,
        assets: &Assets,
    ) {
        if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == Some(bullet_id)) {
            bullet.position = position;
            bullet.velocity = velocity;
            bullet.last_update_tick = tick;
            bullet.blend_ticks_left = 0;
            bullet.blend_step = Vec3::ZERO;
        }

        if let Some(player) = self.players.get_mut(target_index) {
            player.health = target_health;
        }

        if target_health == 0 {
            if target_index == self.local_player_index {
                play_sound_once(&assets.bell_sound);
            } else {
                play_sound_once(&assets.shatter_sound);
            }
        } else if target_index == self.local_player_index {
            play_sound_once(&assets.deep_clang);
        } else {
            play_sound_once(&assets.clang);
        }

        if target_index == self.local_player_index {
            if target_health == 0 {
                self.fade_to_black = Some(fade::new_fade_to_black());
                self.fade_to_black_finished = false;
                if self.obe_effect.is_none() {
                    self.obe_effect = Some(ObeEffect::new(self.players[target_index].state));
                }
            } else {
                self.flash = Some(fade::new_flash());
            }
        }

        if target_health == 0 {
            self.bullets.retain(|b| b.id != Some(bullet_id));
        } else if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == Some(bullet_id)) {
            bullet.position = position;
            bullet.velocity = velocity;
            bullet.last_update_tick = tick;
        }
    }

    fn handle_bullet_expire_event(&mut self, bullet_id: u32) {
        self.bullets.retain(|b| b.id != Some(bullet_id));
    }
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

#[derive(Clone, Copy, Debug, Default)]
struct StaticState {
    position: Vec3,
    pitch: f32,
    yaw: f32,
}

impl StaticState {
    pub fn new(player: &Player) -> Self {
        Self {
            position: player.state.position,
            pitch: player.state.pitch,
            yaw: player.state.yaw,
        }
    }
}
