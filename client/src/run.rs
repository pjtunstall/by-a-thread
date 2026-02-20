use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

use crate::{
    assets::Assets,
    exit, game,
    game::world::sky,
    info,
    lobby::{
        self,
        ui::{Gui, LobbyUi},
    },
    net::{DisconnectKind, RenetNetworkHandle},
    post_game_chat,
    session::{ClientSession, Clock},
    state::{ClientState, Lobby},
};
use common::constants::TICK_SECS;

pub enum RunClientReturn {
    Exit,
    ReturnToStartMenu {
        session: ClientSession,
        ui: Gui,
        assets: Assets,
    },
    ConnectionError {
        message: String,
        session: ClientSession,
        ui: Gui,
        assets: Assets,
    },
}

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
        connect_token: ConnectToken,
        ui: Gui,
        session: ClientSession,
        assets: Assets,
    ) -> Result<Self, (String, Gui, ClientSession, Assets)> {
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before unix epoch");
        if let Err(e) = socket.set_nonblocking(true) {
            return Err((
                format!("failed to set socket as non-blocking: {}", e),
                ui,
                session,
                assets,
            ));
        }

        let authentication = ClientAuthentication::Secure { connect_token };
        let transport =
            match NetcodeClientTransport::new(current_time_duration, authentication, socket) {
                Ok(t) => t,
                Err(e) => {
                    let error_msg = e.to_string();
                    let message = if error_msg.contains("invalid protocol id")
                        || error_msg.contains("invalid version info")
                    {
                        "Version mismatch: server updated. Please download the latest version."
                            .to_string()
                    } else if error_msg.contains("connection denied") {
                        "connection denied: server full or access restricted".to_string()
                    } else if error_msg.contains("connection timed out") {
                        "connection timed out: server not responding".to_string()
                    } else {
                        format!("failed to create network transport: {}", e)
                    };
                    return Err((message, ui, session, assets));
                }
            };
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
            eprintln!("transport update error: {:?}", e);
            result = Err(format!("transport update failed: {}", e));
        }

        self.client.update(dt);

        {
            let mut network_handle = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
            crate::time::estimate_server_clock(&mut self.session, &mut network_handle, dt);
        }

        if let Err(e) = self.transport.send_packets(&mut self.client) {
            eprintln!("transport send_packets error: {:?}", e);
            result = Err(format!("packet send failed: {}", e));
        }

        match result {
            Ok(()) => {}
            Err(e) => {
                let message = disconnect_message(
                    &self.session.state,
                    &e,
                    crate::net::map_disconnect_kind(
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
            self.ui.show_message(" ");
            self.ui
                .show_warning("Press ESCAPE to exit, ENTER to return to menu.");
            eprintln!("disconnected: {}{}", disconnect_message, separator);
            self.session.disconnected_notified = true;
        }

        self.ui.draw(
            false,
            false,
            Some(&self.assets.font),
            None::<crate::lobby::ui::LobbyTimerInfo>,
        );
    }

    fn update_client_state(&mut self) {
        // We can't call `self.display_disconnect_message` in the `match` block
        // below because both mutably borrow `self` (the `ClientRunner`). Hence
        // we handle the disconnected states here separately from the other
        // states. We extract what we need so the borrow of `session.state`
        // ends before we call methods that need `&mut self`.
        let disconnect_message = match &self.session.state {
            ClientState::Disconnected { message } => Some(message.clone()),
            _ => None,
        };
        if let Some(msg) = disconnect_message {
            self.display_disconnect_message(&msg);
            return;
        }
        if matches!(&self.session.state, ClientState::EndAfterLeaderboard) {
            self.ui.draw(
                false,
                false,
                Some(&self.assets.font),
                None::<crate::lobby::ui::LobbyTimerInfo>,
            );
            return;
        }

        match &mut self.session.state {
            ClientState::Game(game_state) => {
                Self::update_sim_clock(&mut self.session.clock, self.frame_dt);

                let mut network = RenetNetworkHandle::new(&mut self.client, &mut self.transport);

                let next_state = game_state.update_with_network(
                    &mut self.session.clock,
                    &mut network,
                    &self.assets,
                );
                match next_state {
                    Some(ClientState::PostGameChat(chat_state)) => {
                        self.ui.flush_input();
                        let old =
                            std::mem::replace(&mut self.session.state, ClientState::default());
                        if let ClientState::Game(game) = old {
                            let full_chat = game.consume_for_post_game(chat_state);
                            self.session.state = ClientState::PostGameChat(full_chat);
                        } else {
                            unreachable!("transition to PostGameChat only happens from Game");
                        }
                    }
                    Some(other) => {
                        self.session.transition(other);
                    }
                    None => {
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
            ClientState::PreLobby(_) => {
                unreachable!(
                    "`PreLobby` is handled by `run_pre_lobby_loop` before `ClientRunner` exists"
                )
            }
            ClientState::Lobby(_) => lobby::state_handlers::update(self),
            ClientState::PostGameChat { .. } => {
                let mut network = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
                if let Some(next_state) = post_game_chat::update(
                    &mut self.session,
                    &mut self.ui,
                    &mut network,
                    Some(&self.assets),
                ) {
                    self.session.transition(next_state);
                }
            }
            ClientState::Disconnected { .. }
            | ClientState::EndAfterLeaderboard
            | ClientState::Transitioning => {}
        }

        if !self.session.state.is_disconnected() {
            if let Some(msg) = self.session.take_pending_disconnect() {
                let next_state = match &self.session.state {
                    ClientState::PostGameChat(crate::post_game_chat::PostGameChat {
                        leaderboard_received: true,
                        ..
                    }) => ClientState::EndAfterLeaderboard,
                    _ => ClientState::Disconnected { message: msg },
                };
                self.session.transition(next_state);
            }
        }
    }

    pub fn start_game(&mut self) -> Result<(), ()> {
        self.session.clock.continuous_sim_time = self.session.clock.estimated_server_time;
        let sim_tick = crate::time::tick_from_time(self.session.clock.estimated_server_time);
        self.session.clock.sim_tick = sim_tick;
        self.last_updated = Instant::now();

        let (initial_data, maze_meshes, map_overlay, sky_mesh) = match &mut self.session.state {
            ClientState::Lobby(Lobby::Countdown {
                game_data,
                maze_meshes,
                map_overlay,
                sky_mesh,
                ..
            }) => (
                std::mem::take(game_data),
                maze_meshes.take(),
                map_overlay.take(),
                std::mem::replace(sky_mesh, sky::generate_sky(None, sky::sky_colors(1))), // Default to level 1.
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
        let map_overlay = map_overlay.expect("map overlay should be built during countdown");

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

        let timer_markers = info::circles::TimerMarkers::new(info::BASE_CIRCLE_RADIUS);
        let needle_textures = info::circles::NeedleTextures::new(info::BASE_CIRCLE_RADIUS);

        self.session.local_player_index = Some(local_player_index);
        self.session
            .transition(ClientState::Game(game::state::Game::new(
                local_player_index,
                initial_data,
                maze_meshes,
                map_overlay,
                sky_mesh,
                sim_tick,
                timer_markers,
                self.session.clock.estimated_server_time,
                needle_textures,
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

pub async fn run_client_loop(
    server_addr: SocketAddr,
    connect_token: ConnectToken,
    share_passcode: Option<String>,
    session: ClientSession,
    ui: Gui,
    assets: Assets,
    only_player: bool,
) -> RunClientReturn {
    println!("Connecting to server: {}", server_addr);

    #[cfg(target_os = "windows")]
    let socket_addr = {
        if server_addr.ip().is_loopback() {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        } else {
            let local_ip = get_best_local_binding_ip();
            SocketAddr::new(local_ip, 0)
        }
    };

    #[cfg(not(target_os = "windows"))]
    let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
    let socket = match UdpSocket::bind(socket_addr) {
        Ok(socket) => socket,
        Err(e) => {
            return RunClientReturn::ConnectionError {
                message: format!("failed to bind client socket: {}", e),
                session,
                ui,
                assets,
            };
        }
    };

    let mut runner = match ClientRunner::new(socket, connect_token, ui, session, assets).await {
        Ok(r) => r,
        Err((message, ui, session, assets)) => {
            return RunClientReturn::ConnectionError {
                message,
                session,
                ui,
                assets,
            };
        }
    };

    runner.ui.print_client_banner(
        env!("CARGO_PKG_VERSION"),
        server_addr,
        share_passcode,
        only_player,
    );

    let mut return_to_menu = false;
    loop {
        let can_return_to_menu = matches!(
            &runner.session.state,
            ClientState::Disconnected { .. }
                | ClientState::EndAfterLeaderboard
                | ClientState::PostGameChat(crate::post_game_chat::PostGameChat {
                    leaderboard_received: true,
                    ..
                },)
        );
        if can_return_to_menu && is_key_pressed(KeyCode::Enter) {
            return_to_menu = true;
            break;
        }
        if exit::should_quit() {
            break;
        }

        runner.session.clock.fps.update();
        runner.pump_network();
        runner.update_client_state();

        next_frame().await;
    }

    if return_to_menu {
        RunClientReturn::ReturnToStartMenu {
            session: ClientSession::new(runner.session.client_id, None),
            ui: runner.ui,
            assets: runner.assets,
        }
    } else {
        RunClientReturn::Exit
    }
}

#[cfg(target_os = "windows")]
fn get_best_local_binding_ip() -> IpAddr {
    // Try to connect to Google's DNS server (8.8.8.8) on port 53.
    // This won't actually send data, just determines the local interface.
    match UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)) {
        Ok(socket) => {
            if socket.connect("8.8.8.8:53").is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    return addr.ip();
                }
            }
        }
        Err(_) => {}
    }

    // Fallback to all interfaces when the dummy connection fails (e.g. firewall
    // blocks 8.8.8.8, or no network). Binding to 127.0.0.1 would prevent receiving
    // packets from remote servers.
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
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
        ClientState::PostGameChat { .. }
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
