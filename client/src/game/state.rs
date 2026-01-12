use std::{
    fmt,
    time::{Duration, Instant},
};

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use macroquad::{color, prelude::*, window::clear_background};

use crate::{
    assets::Assets,
    fade::{self, Fade},
    frame::FrameRate,
    game::world::maze::{MazeExtension, MazeMeshes},
    info,
    net::NetworkHandle,
    state::ClientState,
    time::INTERPOLATION_DELAY_SECS,
};
use common::{
    constants::{INPUT_HISTORY_LENGTH, SNAPSHOT_BUFFER_LENGTH},
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
}

impl Game {
    pub fn new(
        local_player_index: usize,
        initial_data: InitialData,
        maze_meshes: MazeMeshes,
        sim_tick: u64,
        info_map: info::map::MapOverlay,
    ) -> Self {
        Self {
            local_player_index,
            maze: initial_data.maze,
            maze_meshes,
            players: initial_data.players,
            info_map,
            input_history: Ring::new(),

            // `snapshot_buffer.head` will be reset when the first snapshot is
            // inserted, but still we need an initial `head` that's within ±2^15
            // ticks of the tick on which the first snapshot was sent so that
            // the first snapshot's 16-bit wire id will be extended to the
            // correct 64-bit storage id.
            snapshot_buffer: NetworkBuffer::new(sim_tick, 0),

            is_first_snapshot_received: false,
            last_reconciled_tick: None,
            bullets: Vec::new(),
            flash: None,
            fade_to_black: None,
            fade_to_black_finished: false,
        }
    }

    pub fn send_input(
        &mut self,
        network: &mut dyn NetworkHandle,
        input: PlayerInput,
        sim_tick: u64,
    ) {
        let wire_tick: u16 = sim_tick as u16;
        let wire_input = WireItem {
            id: wire_tick,
            data: input,
        };
        let client_message = ClientMessage::Input(wire_input);
        let payload =
            encode_to_vec(&client_message, standard()).expect("failed to encode player input");
        network.send_message(AppChannel::Unreliable, payload);
        // println!("{:?}", client_message);
    }

    // TODO: Consider disparity in naming between snapshot as data without id,
    // and snapshot as WireItem together with id.
    pub fn receive_snapshots(&mut self, network: &mut dyn NetworkHandle) {
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
                    self.apply_bullet_event(event);
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
                    self.apply_bullet_event(event);
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

    pub fn apply_input_range(&mut self, from: u64, to: u64) {
        for tick in from..=to {
            self.apply_input(tick);
        }
    }

    pub fn apply_input(&mut self, tick: u64) {
        if let Some(input) = self.input_history.get(tick) {
            self.players[self.local_player_index]
                .state
                .update(&self.maze, input);
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

            debug_assert!(remote_a.len() == 3);
            debug_assert!(remote_b.len() == 3);
            let Some(a) = remote_a.get(remote_index) else {
                // With that assumption, we should never get here...
                return None;
            };
            let Some(b) = remote_b.get(remote_index) else {
                // ...or here.
                return None;
            };

            let state = &mut player.state;
            state.position = a.position + (b.position - a.position) * alpha;
            state.yaw = a.yaw + (b.yaw - a.yaw) * alpha;
            state.pitch = a.pitch + (b.pitch - a.pitch) * alpha;

            remote_index += 1;
        }

        // We subtract a wide safety margin in case `estimated_server_time` goes
        // back temporarily backwards due to network fluctuation.
        Some(tick_a - 60)
    }

    // TODO: Handle possible change of state to post-game. That would be due to
    // collision with bullets, which will be sent on the reliable channel from
    // the server. I'll see whether this function is needed when I
    // implement that, or whether the state change is best placed elsewhere.
    pub fn update(&mut self, sim_tick: u64) -> Option<ClientState> {
        self.update_bullets(sim_tick);
        None
    }

    // TODO: `prediction_alpha` would be for smoothing the local player between
    // ticks if I allow faster than 60Hz frame rate for devices that support it.
    pub fn draw(&mut self, _prediction_alpha: f64, assets: &Assets, fps: &FrameRate) {
        clear_background(color::BEIGE);

        let i = self.local_player_index;
        let position = self.players[i].state.position;
        let yaw = self.players[i].state.yaw;
        let pitch = self.players[i].state.pitch;

        set_camera(&Camera3D {
            position,
            target: position
                + vec3(
                    -yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                ),
            up: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        });

        self.maze.draw(&self.maze_meshes);
        self.draw_remote_players(assets);
        self.draw_bullets();
        // self.draw_test_sphere(assets);

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

    fn draw_remote_players(&self, assets: &Assets) {
        for (index, player) in self.players.iter().enumerate() {
            if index == self.local_player_index {
                continue;
            }

            draw_sphere(
                player.state.position,
                player::RADIUS,
                Some(&assets.ball_texture),
                WHITE,
            );
        }
    }

    fn draw_bullets(&self) {
        const BULLET_DRAW_RADIUS: f32 = 4.0;

        for bullet in &self.bullets {
            draw_sphere(bullet.position, BULLET_DRAW_RADIUS, None, WHITE);
        }
    }

    fn update_bullets(&mut self, sim_tick: u64) {
        for bullet in &mut self.bullets {
            if sim_tick <= bullet.last_update_tick {
                continue;
            }

            let ticks = sim_tick - bullet.last_update_tick;
            bullet.advance(ticks);
            bullet.last_update_tick = sim_tick;
        }
    }

    fn apply_bullet_event(&mut self, event: BulletEvent) {
        match event {
            BulletEvent::Spawn {
                bullet_id,
                tick,
                position,
                velocity,
            } => {
                self.bullets
                    .push(ClientBullet::new(bullet_id, position, velocity, tick));
            }
            BulletEvent::Bounce {
                bullet_id,
                tick,
                position,
                velocity,
            } => {
                if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == bullet_id) {
                    bullet.position = position;
                    bullet.velocity = velocity;
                    bullet.last_update_tick = tick;
                }
            }
            BulletEvent::Hit {
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
                    } else {
                        self.flash = Some(fade::new_flash());
                    }
                }

                if target_health == 0 {
                    self.bullets.retain(|b| b.id != bullet_id);
                } else if let Some(bullet) = self.bullets.iter_mut().find(|b| b.id == bullet_id) {
                    bullet.position = position;
                    bullet.velocity = velocity;
                    bullet.last_update_tick = tick;
                }
            }
            BulletEvent::Expire { bullet_id, .. } => {
                self.bullets.retain(|b| b.id != bullet_id);
            }
        }
    }

    // fn draw_test_sphere(&self, assets: &Assets) {
    //     let local_player = &self.players[self.local_player_index];
    //     let yaw = local_player.state.yaw;
    //     let pitch = local_player.state.pitch;
    //     let forward = vec3(
    //         -yaw.sin() * pitch.cos(),
    //         pitch.sin(),
    //         -yaw.cos() * pitch.cos(),
    //     );
    //     let position = local_player.state.position + forward * player::RADIUS * 6.0;

    //     draw_sphere(position, player::RADIUS, Some(&assets.ball_texture), WHITE);
    // }
}

#[derive(Debug, Clone, Copy)]
pub struct ClientBullet {
    pub id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub last_update_tick: u64,
}

impl ClientBullet {
    pub fn new(id: u32, position: Vec3, velocity: Vec3, last_update_tick: u64) -> Self {
        Self {
            id,
            position,
            velocity,
            last_update_tick,
        }
    }

    pub fn advance(&mut self, ticks: u64) {
        let delta = ticks as f32 * common::constants::TICK_SECS_F32;
        self.position += self.velocity * delta;
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
