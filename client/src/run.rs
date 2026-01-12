use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    assets::Assets,
    game::{self, input},
    info,
    lobby::{
        self,
        ui::{Gui, LobbyUi},
    },
    net::{self, DisconnectKind, RenetNetworkHandle},
    session::{ClientSession, Clock},
    state::{ClientState, Lobby},
};
use common::{self, constants::TICK_SECS, player::Player};

pub struct ClientRunner {
    pub session: ClientSession,
    pub client: RenetClient,
    pub transport: NetcodeClientTransport,
    pub ui: Gui,
    pub assets: Assets,
    last_updated: Instant,
    frame_dt: Duration,
}

impl ClientRunner {
    pub async fn new(
        socket: UdpSocket,
        server_addr: SocketAddr,
        private_key: [u8; 32],
        ui: Gui,
    ) -> Result<Self, String> {
        let assets = Assets::load().await;
        let client_id = ::rand::random::<u64>();
        let protocol_id = common::protocol::version();
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before unix epoch");
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("failed to set socket as non-blocking: {}", e))?;
        let connect_token = net::create_connect_token(
            current_time_duration,
            protocol_id,
            client_id,
            server_addr,
            &private_key,
        );
        let authentication = ClientAuthentication::Secure { connect_token };
        let transport = NetcodeClientTransport::new(current_time_duration, authentication, socket)
            .map_err(|e| format!("failed to create network transport: {}", e))?;
        let connection_config = common::net::connection_config();
        let client = RenetClient::new(connection_config);
        let session = ClientSession::new(client_id);

        Ok(Self {
            session,
            client,
            transport,
            ui,
            last_updated: Instant::now(),
            frame_dt: Duration::ZERO,
            assets,
        })
    }

    pub fn pump_network(&mut self) {
        if self.session.state().is_disconnected() {
            return;
        }

        let now = Instant::now();
        let dt = now - self.last_updated;
        self.frame_dt = dt;
        self.last_updated = now;

        let mut result: Result<(), String> = Ok(());

        if let Err(e) = self.transport.update(dt, &mut self.client) {
            result = Err(format!("transport update failed: {}", e));
        }

        self.client.update(dt);

        {
            let mut network_handle = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
            crate::time::estimate_server_clock(&mut self.session, &mut network_handle, dt);
        }

        if let Err(e) = self.transport.send_packets(&mut self.client) {
            result = Err(format!("packet send failed: {}", e));
        }

        match result {
            Ok(()) => {}
            Err(e) => {
                let message = disconnect_message(
                    self.session.state(),
                    &e,
                    net::map_disconnect_kind(
                        self.client.disconnect_reason(),
                        self.transport.disconnect_reason(),
                    ),
                );
                self.session.set_pending_disconnect(message);
            }
        }
    }

    fn display_disconnect_message(&mut self, disconnect_message: &str) {
        if !self.session.disconnected_notified {
            let separator = if disconnect_message
                .chars()
                .last()
                .is_some_and(|c| ['.', '!', '?'].contains(&c))
            {
                ""
            } else {
                "."
            };
            self.ui.show_sanitized_error(&format!(
                "Disconnected: {}{}",
                &disconnect_message, separator
            ));
            eprintln!("disconnected: {}{}", disconnect_message, separator);
            self.session.disconnected_notified = true;
        }

        self.ui.draw(false, false, Some(&self.assets.font));
    }

    fn update_client_state(&mut self) {
        if let Some(disconnect_message) = {
            if let ClientState::Disconnected { message } = self.session.state() {
                Some(message.clone())
            } else {
                None
            }
        } {
            self.display_disconnect_message(&disconnect_message);
            return;
        }

        match &mut self.session.state {
            ClientState::Game(game_state) => {
                Self::update_sim_clock(&mut self.session.clock, self.frame_dt);

                let mut network = RenetNetworkHandle::new(&mut self.client, &mut self.transport);

                game_state.receive_snapshots(&mut network);

                match Self::advance_simulation(&mut self.session.clock, &mut network, game_state) {
                    Some(next_state) => {
                        self.session.transition(next_state);
                    }
                    _ => {
                        // TODO: `prediction_alpha` would be for smoothing the
                        // local player between ticks if I allow faster than
                        // 60Hz frame rate for devices that support it. As yet,
                        // it's unused in `draw`.
                        let prediction_alpha = self.session.clock.accumulated_time / TICK_SECS;
                        if let Some(new_tail) =
                            game_state.interpolate(self.session.clock.estimated_server_time)
                        {
                            game_state.snapshot_buffer.advance_tail(new_tail);
                        }

                        game_state.draw(prediction_alpha, &self.assets, &self.session.clock.fps);
                    }
                }
            }
            // TODO: Following the pattern of the game handler, pass inner state
            // to each of the lobby substate handlers so as to let the type
            // system enforce that the correct type is sent, rather than having
            // explicit guards at the start of each handler. This will mean
            // passing the inner state to the handler, rather than passing
            // `session`.`
            ClientState::Lobby(_) => lobby::handlers::update(self),
            // Other states will include Debrief (with map, leaderboard, and chat),
            // and NearDeathExperience, unless the latter is included in Game.
            _ => {
                todo!();
            }
        }

        if !self.session.state().is_disconnected() {
            if let Some(message) = self.session.take_pending_disconnect() {
                self.session
                    .transition(ClientState::Disconnected { message });
            }
        }
    }

    pub fn start_game(&mut self) -> Result<(), ()> {
        self.session.clock.continuous_sim_time = self.session.clock.estimated_server_time;
        let sim_tick = crate::time::tick_from_time(self.session.clock.estimated_server_time);
        self.session.clock.sim_tick = sim_tick;
        self.last_updated = Instant::now();

        let (initial_data, maze_meshes) = match self.session.state_mut() {
            ClientState::Lobby(Lobby::Countdown {
                game_data,
                maze_meshes,
                ..
            }) => (std::mem::take(game_data), maze_meshes.take()),
            other => {
                self.ui.show_sanitized_error(&format!(
                    "Tried to start game from invalid state: {:#?}.",
                    other
                ));
                return Err(());
            }
        };

        let maze_meshes = maze_meshes.expect("maze meshes should be built during countdown");
        let info_map = info::map::initialize_map(&initial_data.maze, &self.assets.font);

        let Some(local_player_index) = initial_data
            .players
            .iter()
            .position(|p| p.client_id == self.session.client_id)
        else {
            self.session.transition(ClientState::Disconnected {
                message: format!("could not find you in the list of players"),
            });
            return Err(());
        };

        self.session.local_player_index = Some(local_player_index);
        self.session
            .transition(ClientState::Game(game::state::Game::new(
                local_player_index,
                initial_data,
                maze_meshes,
                sim_tick,
                info_map,
            )));

        Ok(())
    }

    fn update_sim_clock(clock: &mut Clock, frame_dt: Duration) {
        let target_time =
            crate::time::calculate_target_time(clock.smoothed_rtt, clock.estimated_server_time);
        let frame_dt_secs = frame_dt
            .as_secs_f64()
            // Clamp to avoid huge jumps if a frame stalls.
            .min(0.25);
        let smoothed_dt =
            crate::time::smooth_dt(clock.continuous_sim_time, target_time, frame_dt_secs);

        clock.accumulated_time += smoothed_dt;
        clock.continuous_sim_time += smoothed_dt;

        // println!("{frame_dt_secs}");
        // println!("{}", clock.sim_tick);
    }

    fn advance_simulation(
        clock: &mut Clock,
        network_handle: &mut RenetNetworkHandle<'_>,
        game_state: &mut game::state::Game,
    ) -> Option<ClientState> {
        let mut transition = None;

        // A failsafe to prevent `accumulated_time` from growing ever greater
        // if we fall behind.
        const MAX_TICKS_PER_FRAME: u8 = 8;
        let mut ticks_processed = 0;

        let head = game_state.snapshot_buffer.head;
        if game_state.reconcile(head) {
            let start_replay = head + 1;
            let end_replay = clock.sim_tick + 1;

            if start_replay <= end_replay {
                game_state.apply_input_range_inclusive(start_replay, end_replay);
            }
        }

        while clock.accumulated_time >= TICK_SECS && ticks_processed < MAX_TICKS_PER_FRAME {
            let sim_tick = clock.sim_tick;
            let input = input::player_input_from_keys(sim_tick);
            game_state.send_input(network_handle, input, sim_tick);
            game_state.input_history.insert(sim_tick, input);
            game_state.apply_input(sim_tick);
            transition = game_state.update(sim_tick);

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
        transition
    }
}

pub async fn run_client_loop(
    socket: UdpSocket,
    server_addr: SocketAddr,
    private_key: [u8; 32],
    ui: Gui,
) {
    let mut runner = match ClientRunner::new(socket, server_addr, private_key, ui).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    runner.ui.print_client_banner(
        common::protocol::version(),
        server_addr,
        runner.session.client_id,
    );

    loop {
        if is_quit_requested() || is_key_pressed(KeyCode::Escape) {
            break;
        }

        runner.session.clock.fps.update();
        // println!("{}", runner.session.clock.fps.rate);
        runner.pump_network();
        runner.update_client_state();

        next_frame().await;
    }
}

// TODO: Remove this function eventually if it stays unused, along with the
// strum crate and derivation of macro for turning the `Color` enum into a
// `String`.
pub fn print_player_list(ui: &mut dyn LobbyUi, session: &ClientSession, players: &Vec<Player>) {
    ui.show_message("\nPlayers:");
    for player in players {
        let is_self = if player.client_id == session.client_id {
            "<--you"
        } else {
            ""
        };
        ui.show_sanitized_message(&format!(
            " - {} ({}) {}",
            player.name,
            player.color.to_string(),
            is_self
        ));
    }
    ui.show_sanitized_message("");
}

fn disconnect_message(state: &ClientState, error: &str, kind: DisconnectKind) -> String {
    if let ClientState::Lobby(lobby_state) = state {
        match lobby_state {
            Lobby::Connecting { .. }
                if matches!(
                    kind,
                    DisconnectKind::DisconnectedByServer | DisconnectKind::ConnectionDenied
                ) =>
            {
                return common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string();
            }
            Lobby::Startup { .. }
                if matches!(
                    kind,
                    DisconnectKind::DisconnectedByServer | DisconnectKind::ConnectionDenied
                ) =>
            {
                return common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string();
            }
            Lobby::Authenticating { .. }
                if matches!(kind, DisconnectKind::DisconnectedByServer) =>
            {
                return "authentication failed: server closed the connection".to_string();
            }
            Lobby::AwaitingUsernameConfirmation => {
                return format!(
                    "disconnected while awaiting username confirmation: {}",
                    error
                );
            }
            Lobby::Chat { .. } if matches!(kind, DisconnectKind::DisconnectedByServer) => {
                return "disconnected from lobby: server closed the connection".to_string();
            }
            _ => {}
        }
    }

    format!("no connection: {}", error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby::state::Lobby;

    #[test]
    fn disconnect_message_for_connecting_when_server_terminates() {
        let state = ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        });
        let msg = disconnect_message(
            &state,
            "connection terminated by server",
            DisconnectKind::DisconnectedByServer,
        );
        assert_eq!(
            msg,
            common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string()
        );
    }

    #[test]
    fn disconnect_message_for_startup_when_server_denies() {
        let state = ClientState::Lobby(Lobby::Startup {
            prompt_printed: true,
        });
        let msg = disconnect_message(
            &state,
            "DisconnectedByServer",
            DisconnectKind::DisconnectedByServer,
        );
        assert_eq!(
            msg,
            common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string()
        );
    }

    #[test]
    fn disconnect_message_for_authentication_server_close() {
        let state = ClientState::Lobby(Lobby::Authenticating {
            waiting_for_input: true,
            waiting_for_server: false,
            guesses_left: 3,
        });
        let msg = disconnect_message(
            &state,
            "connection terminated by server",
            DisconnectKind::DisconnectedByServer,
        );
        assert_eq!(
            msg,
            "authentication failed: server closed the connection".to_string()
        );
    }

    #[test]
    fn disconnect_message_for_username_confirmation_disconnect() {
        let state = ClientState::Lobby(Lobby::AwaitingUsernameConfirmation);
        let msg = disconnect_message(
            &state,
            "timeout",
            DisconnectKind::Other("timeout".to_string()),
        );
        assert_eq!(
            msg,
            "disconnected while awaiting username confirmation: timeout".to_string()
        );
    }

    #[test]
    fn disconnect_message_defaults_when_no_special_case() {
        let state = ClientState::Debrief;
        let msg = disconnect_message(
            &state,
            "some error",
            DisconnectKind::Other("some error".to_string()),
        );
        assert_eq!(msg, "no connection: some error".to_string());
    }
}
