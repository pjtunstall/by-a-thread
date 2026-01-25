use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    after_game_chat,
    assets::Assets,
    game,
    game::world::sky,
    info,
    lobby::{
        self,
        ui::{Gui, LobbyUi},
    },
    net::{self, DisconnectKind, RenetNetworkHandle},
    session::{ClientSession, Clock},
    state::{ClientState, InputMode, Lobby},
};
use common::{self, constants::TICK_SECS};

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
        session: ClientSession,
        assets: Assets,
    ) -> Result<Self, String> {
        let protocol_id = common::protocol::version();
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before unix epoch");
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("failed to set socket as non-blocking: {}", e))?;

        // TODO: In production, the client should receive this token from a matchmaker.
        let connect_token = net::create_connect_token(
            current_time_duration,
            protocol_id,
            session.client_id,
            server_addr,
            &private_key,
        );
        let authentication = ClientAuthentication::Secure { connect_token };
        let transport = NetcodeClientTransport::new(current_time_duration, authentication, socket)
            .map_err(|e| {
                let error_msg = e.to_string();
                if error_msg.contains("invalid protocol id")
                    || error_msg.contains("invalid version info")
                {
                    "version mismatch: client and server versions do not match".to_string()
                } else if error_msg.contains("connection denied") {
                    "connection denied: server full or access restricted".to_string()
                } else if error_msg.contains("connection timed out") {
                    "connection timed out: server not responding".to_string()
                } else {
                    format!("failed to create network transport: {}", e)
                }
            })?;
        let connection_config = common::net::connection_config();
        let client = RenetClient::new(connection_config);

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
        if self.session.state.is_disconnected() {
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
                    &self.session.state,
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
        // We can't call `self.display_disconnect_message` in the `match` block
        // below because both mutably borrow `self` (the `ClientRunner`). Hence
        // we handle the `Disconnected` state here separately from the other
        // states.
        if let Some(disconnect_message) = {
            if let ClientState::Disconnected { message } = &self.session.state {
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

                match game_state.update_with_network(
                    &mut self.session.clock,
                    &mut network,
                    &self.assets,
                ) {
                    Some(next_state) => {
                        if matches!(next_state, ClientState::AfterGameChat { .. }) {
                            self.ui.flush_input();
                        }
                        self.session.transition(next_state);
                    }
                    _ => {
                        let tick_fraction = (self.session.clock.accumulated_time / TICK_SECS)
                            .clamp(0.0, 1.0) as f32;
                        game_state.draw(
                            tick_fraction,
                            &self.assets,
                            &self.session.clock.fps,
                            self.session.clock.estimated_server_time,
                        );
                    }
                }
            }
            ClientState::Lobby(_) => lobby::state_handlers::update(self),
            ClientState::AfterGameChat { .. } => {
                let mut network = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
                if let Some(next_state) = after_game_chat::update(
                    &mut self.session,
                    &mut self.ui,
                    &mut network,
                    Some(&self.assets),
                ) {
                    self.session.transition(next_state);
                }
            }
            ClientState::Disconnected { .. } => {}
        }

        if !self.session.state.is_disconnected() {
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

        let (initial_data, maze_meshes, sky_mesh) = match &mut self.session.state {
            ClientState::Lobby(Lobby::Countdown {
                game_data,
                maze_meshes,
                sky_mesh,
                ..
            }) => (
                std::mem::take(game_data),
                maze_meshes.take(),
                std::mem::replace(sky_mesh, sky::generate_sky(None, sky::sky_colors(1))), // Default to level 1
            ),
            other => {
                self.ui.show_sanitized_error(&format!(
                    "Tried to start game from invalid state: {:#?}.",
                    other
                ));
                return Err(());
            }
        };

        let maze_meshes = maze_meshes.expect("maze meshes should be built during countdown");
        let info_map = info::map::initialize_map(&initial_data.maze, &self.assets.map_font);

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
                sky_mesh,
                sim_tick,
                info_map,
                self.session.clock.estimated_server_time,
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
    }
}

pub async fn run_client_loop(private_key: [u8; 32], mut ui: Gui) {
    let client_id = ::rand::random::<u64>();
    let mut session = ClientSession::new(client_id);
    let assets = Assets::load().await;
    let Some(server_addr) =
        prompt_for_server_address(&mut session, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    println!("Connecting to server: {}", server_addr);

    let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
    let socket = match UdpSocket::bind(socket_addr) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("failed to bind client socket: {}", e);
            return;
        }
    };

    let mut runner =
        match ClientRunner::new(socket, server_addr, private_key, ui, session, assets).await {
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

async fn prompt_for_server_address(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    font: Option<&Font>,
) -> Option<SocketAddr> {
    loop {
        if is_quit_requested() || is_key_pressed(KeyCode::Escape) {
            return None;
        }

        if matches!(session.input_mode(), InputMode::Enabled) {
            match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
                Ok(Some(input)) => session.add_input(input),
                Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                    ui.show_sanitized_error(&format!("No connection: {}.", e));
                    return None;
                }
                Ok(None) => {}
            }
        }

        let state = std::mem::take(&mut session.state);
        let result = match state {
            ClientState::Lobby(mut lobby_state) => {
                let result =
                    lobby::state_handlers::server_address::handle(&mut lobby_state, session, ui);
                session.state = ClientState::Lobby(lobby_state);
                result
            }
            other_state => {
                session.state = other_state;
                None
            }
        };

        if let Some(next_state) = result {
            session.transition(next_state);
        }

        if let Some(server_addr) = session.server_addr {
            ui.flush_input();
            return Some(server_addr);
        }

        let ui_state = session.prepare_ui_state();
        if ui_state.show_waiting_message {
            ui.show_warning("Waiting for server...");
        }

        let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
        let show_cursor = should_show_input;
        ui.draw(should_show_input, show_cursor, font);

        next_frame().await;
    }
}

fn disconnect_message(state: &ClientState, error: &str, kind: DisconnectKind) -> String {
    match state {
        ClientState::Lobby(lobby_state) => match lobby_state {
            Lobby::Connecting { .. }
                if matches!(
                    kind,
                    DisconnectKind::DisconnectedByServer | DisconnectKind::ConnectionDenied
                ) =>
            {
                return common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string();
            }
            Lobby::Passcode { .. }
                if matches!(
                    kind,
                    DisconnectKind::DisconnectedByServer | DisconnectKind::ConnectionDenied
                ) =>
            {
                return common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string();
            }
            Lobby::ServerAddress { .. }
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
        },
        ClientState::AfterGameChat { .. }
            if matches!(kind, DisconnectKind::DisconnectedByServer) =>
        {
            return "disconnected from chat: server closed the connection".to_string();
        }
        _ => {}
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
    fn disconnect_message_for_passcode_when_server_denies() {
        let state = ClientState::Lobby(Lobby::Passcode {
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
}
