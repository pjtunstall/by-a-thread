use std::fmt;

use bincode::{config::standard, serde::encode_to_vec};
use macroquad::{color, prelude::*, window::clear_background};

use crate::{
    game::{
        input::player_input_from_keys,
        world::maze::{MazeExtension, MazeMeshes},
    },
    net::NetworkHandle,
    state::ClientState,
};
use common::{
    constants::{INPUT_HISTORY_LENGTH, SNAPSHOT_BUFFER_LENGTH},
    maze::Maze,
    net::AppChannel,
    player::{Player, PlayerInput},
    protocol::ClientMessage,
    ring::WireItem,
    ring::{NetworkBuffer, Ring},
    snapshot::{InitialData, Snapshot},
};

pub struct Game {
    pub local_player_index: usize,
    pub maze: Maze,
    pub maze_meshes: MazeMeshes,
    pub players: Vec<Player>,
    pub input_history: Ring<PlayerInput, INPUT_HISTORY_LENGTH>, // 256: ~4.3s at 60Hz.
    pub snapshot_buffer: NetworkBuffer<Snapshot, SNAPSHOT_BUFFER_LENGTH>, // 16 broadcasts, 0.8s at 20Hz.
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
            // TODO: Decide on initial head and tail. The server's
            // `input_buffer` uses `current_tick` for both. The danger is that
            // when a new snapshot arrives, if the tail is at 0, the 16-bit id
            // will be mapped to 64-bit id close to 0 and store it with that
            // wrong id. When we try to get the snapshot for a 64-bit value
            // close to now, the `get` method will map it to an index and see
            // that the snapshot at that index has a different id, one close to
            // 0, and thus think we don't have the right snapshot.
            snapshot_buffer: NetworkBuffer::new(sim_tick, sim_tick),
        }
    }

    pub fn input(&mut self, network: &mut dyn NetworkHandle, sim_tick: u64) {
        let wire_tick: u16 = sim_tick as u16;
        let input = player_input_from_keys(sim_tick);
        let wire_input = WireItem {
            id: wire_tick,
            data: input,
        };
        let client_message = ClientMessage::Input(wire_input);
        let payload =
            encode_to_vec(&client_message, standard()).expect("failed to encode player input");
        network.send_message(AppChannel::Unreliable, payload);
        self.input_history.insert(sim_tick, input);
        // println!("{:?}", client_message);
    }

    pub fn update(&mut self) -> Option<ClientState> {
        // TODO: Reconciliation and prediction.
        None
    }

    pub fn draw(&self, _alpha: f64) {
        clear_background(color::BEIGE);

        let position = self.players[self.local_player_index].state.position;

        let yaw: f32 = 0.0;
        let pitch: f32 = 0.1;

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
