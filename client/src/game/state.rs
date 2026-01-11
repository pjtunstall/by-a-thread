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
    game::world::maze::{MazeExtension, MazeMeshes},
    net::NetworkHandle,
    state::ClientState,
    time::INTERPOLATION_DELAY_SECS,
};
use common::{
    constants::{INPUT_HISTORY_LENGTH, SNAPSHOT_BUFFER_LENGTH},
    maze::Maze,
    net::AppChannel,
    player::{Player, PlayerInput},
    protocol::{ClientMessage, ServerMessage},
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
    pub input_history: Ring<PlayerInput, INPUT_HISTORY_LENGTH>, // 256: ~4.3s at 60Hz.
    pub snapshot_buffer: NetworkBuffer<Snapshot, SNAPSHOT_BUFFER_LENGTH>, // 16 broadcasts, 0.8s at 20Hz.
    pub is_first_snapshot_received: bool,
    pub last_reconciled_tick: Option<u64>,
}

impl Game {
    pub fn new(
        local_player_index: usize,
        initial_data: InitialData,
        maze_meshes: MazeMeshes,
        sim_tick: u64,
    ) -> Self {
        Self {
            local_player_index,
            maze: initial_data.maze,
            maze_meshes,
            players: initial_data.players,
            input_history: Ring::new(),

            // `head` will be reset when the first snapshot is inserted, but
            // still we need an initial `head` that's within ±2^15 ticks of the
            // tick on which the first snapshot was sent so that the first
            // snapshot's 16-bit wire id will be extended to the correct 64-bit
            // storage id.

            // The `tail` (used as a guard against writing outdated items on the
            // `input_buffers`) is not used here due to its minimal advantage.
            // While it could, in principle, be set to `tick_a` of
            // `calculate_interpolation_data`, that doesn't seem worth the
            // trade-offs: looking up the snapshots again in `draw`, copying
            // them, returning a value, or borrowing gymnastics.
            snapshot_buffer: NetworkBuffer::new(sim_tick, 0),

            is_first_snapshot_received: false,
            last_reconciled_tick: None,
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

    pub fn calculate_interpolation_data(
        &self,
        estimated_server_time: f64,
    ) -> Option<(f64, &Snapshot, &Snapshot)> {
        let interpolation_time = estimated_server_time - INTERPOLATION_DELAY_SECS;
        let start_search_tick = crate::time::tick_from_time(interpolation_time);
        let mut a_tick = start_search_tick;
        let limit = 8;

        while self.snapshot_buffer.get(a_tick).is_none() {
            if start_search_tick - a_tick > limit {
                return None;
            };
            a_tick -= 1;
        }

        let mut b_tick = start_search_tick + 1;

        while self.snapshot_buffer.get(b_tick).is_none() {
            if b_tick - (start_search_tick + 1) > limit {
                return None;
            }
            b_tick += 1;
        }

        let snapshot_a = self.snapshot_buffer.get(a_tick)?;
        let snapshot_b = self.snapshot_buffer.get(b_tick)?;

        let a_time = crate::time::time_from_tick(a_tick);
        let b_time = crate::time::time_from_tick(b_tick);
        let alpha = (interpolation_time - a_time) / (b_time - a_time);

        Some((alpha, snapshot_a, snapshot_b))
    }

    // TODO: Handle possible change of state to post-game. That would be due to
    // collision with bullets, which will be sent on the reliable channel from
    // the server. I'll see whether this function is needed when I
    // implement that, or whether the state change is best placed elsewhere.
    pub fn update(&mut self) -> Option<ClientState> {
        None
    }

    // TODO: `prediction_alpha` would be for smoothing the local player between
    // ticks if I allow faster than 60Hz frame rate for devices that support it.
    pub fn draw(
        &self,
        _prediction_alpha: f64,
        _interpolation_data: Option<(f64, &Snapshot, &Snapshot)>,
    ) {
        clear_background(color::BEIGE);

        let i = self.local_player_index;
        let position = self.players[i].state.position;
        let yaw = self.players[i].state.yaw;
        let pitch = self.players[i].state.pitch;

        set_camera(&Camera3D {
            position,
            target: position
                + vec3(
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    yaw.cos() * pitch.cos(),
                ),
            up: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        });

        self.maze.draw(&self.maze_meshes);
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
