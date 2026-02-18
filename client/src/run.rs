use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    api,
    assets::Assets,
    game,
    game::world::sky,
    info,
    lobby::{
        self, state_handlers,
        ui::{Gui, LobbyUi},
    },
    net::{self, DisconnectKind, RenetNetworkHandle},
    post_game_chat,
    session::{ClientSession, Clock},
    state::{ClientState, InputMode, Lobby},
};
use common::player::Color;
use common::{self, auth::Passcode, constants::TICK_SECS};
use renet_netcode::ConnectToken;

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
        connect_token: Option<ConnectToken>,
        private_key: [u8; 32],
        ui: Gui,
        session: ClientSession,
        assets: Assets,
    ) -> Result<Self, String> {
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before unix epoch");
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("failed to set socket as non-blocking: {}", e))?;

        let connect_token = match connect_token {
            Some(token) => token,
            None => {
                let protocol_id = common::protocol::protocol_id();
                net::create_connect_token(
                    current_time_duration,
                    protocol_id,
                    session.client_id,
                    server_addr,
                    &private_key,
                )
            }
        };
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
            self.ui.show_message(" ");
            self.ui.show_warning("Press Escape to exit.");
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
    private_key: [u8; 32],
    server_addr: SocketAddr,
    connect_token: Option<ConnectToken>,
    share_passcode: Option<String>,
    session: ClientSession,
    ui: Gui,
    assets: Assets,
) {
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
            eprintln!("failed to bind client socket: {}", e);
            return;
        }
    };

    let mut runner = match ClientRunner::new(
        socket,
        server_addr,
        connect_token,
        private_key,
        ui,
        session,
        assets,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    runner.ui.print_client_banner(
        common::protocol::version_string(),
        server_addr,
        share_passcode,
    );

    loop {
        if should_quit() {
            break;
        }

        runner.session.clock.fps.update();
        // println!("{}", runner.session.clock.fps.rate);
        runner.pump_network();
        runner.update_client_state();

        next_frame().await;
    }
}

fn should_quit() -> bool {
    is_quit_requested() || is_key_pressed(KeyCode::Escape)
}

async fn await_error_dismissal(ui: &mut dyn LobbyUi, font: Option<&macroquad::prelude::Font>) {
    ui.show_warning("Press Escape to exit.");
    while !should_quit() {
        ui.draw(false, false, font, None);
        next_frame().await;
    }
}

pub async fn prompt_for_server_address(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<String> {
    loop {
        if should_quit() {
            return None;
        }

        if matches!(session.input_mode(), InputMode::Enabled)
            || matches!(session.input_mode(), InputMode::SingleKey)
        {
            match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
                Ok(Some(input)) => session.add_input(input),
                Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                    ui.show_sanitized_error(&format!("No connection: {}.", e));
                    return None;
                }
                Ok(None) => {}
            }
        }

        if let ClientState::Lobby(Lobby::MatchmakerMenu { api_host, .. }) = &session.state {
            ui.flush_input();
            return Some(api_host.clone());
        }

        let state = std::mem::take(&mut session.state);
        let result = match state {
            ClientState::Lobby(mut lobby_state) => {
                let result = match &lobby_state {
                    Lobby::ServerAddress { .. } => {
                        state_handlers::server_address::handle(&mut lobby_state, session, ui)
                    }
                    _ => None,
                };
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

        let ui_state = session.prepare_ui_state();
        if ui_state.show_waiting_message {
            ui.show_warning("Waiting for server...");
        }

        let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
        let show_cursor = should_show_input;
        ui.draw(
            should_show_input,
            show_cursor,
            font,
            None::<crate::lobby::ui::LobbyTimerInfo>,
        );

        next_frame().await;
    }
}

const NEW_JOIN_MENU_ITEMS: &[&str] = &["New game", "Join game"];

fn format_new_join_menu_lines(selected_index: usize) -> Vec<(String, Color)> {
    let mut lines = Vec::with_capacity(4);
    for (i, label) in NEW_JOIN_MENU_ITEMS.iter().enumerate() {
        let prefix = if i == selected_index { "  * " } else { "    " };
        let line_color = if i == selected_index {
            Color::WHITE
        } else {
            Color::LightGray
        };
        lines.push((format!("{}{}", prefix, label), line_color));
    }
    lines.push((" ".to_string(), Color::LightGray));
    lines.push((
        "Use up/down arrows to select, ENTER to confirm.".to_string(),
        Color::LightGray,
    ));
    lines
}

pub async fn prompt_for_matchmaker_choice(
    api_host: &str,
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<(SocketAddr, ConnectToken, Passcode, bool)> {
    let matchmaker_host = Some(api_host);

    const JOIN_GUESS_LIMIT: u8 = 3;

    let mut prompt_printed = false;
    let mut choice_displayed = false;
    let mut choice: Option<u8> = None;
    let mut selected_index: usize = 0;
    let mut wrong_guesses: u8 = 0;

    loop {
        if should_quit() {
            return None;
        }

        if choice.is_none() {
            match ui.poll_single_key() {
                Ok(Some(common::input::UiKey::Up)) => {
                    selected_index = (selected_index + NEW_JOIN_MENU_ITEMS.len() - 1)
                        % NEW_JOIN_MENU_ITEMS.len();
                }
                Ok(Some(common::input::UiKey::Down)) => {
                    selected_index = (selected_index + 1) % NEW_JOIN_MENU_ITEMS.len();
                }
                Ok(Some(common::input::UiKey::Enter)) => {
                    choice = Some((selected_index + 1) as u8);
                    prompt_printed = false;
                }
                Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                    ui.show_sanitized_error(&format!("No connection: {}.", e));
                    await_error_dismissal(ui, font).await;
                    return None;
                }
                _ => {}
            }
        } else {
            match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
                Ok(Some(input)) => {
                    let trimmed = input.trim();
                    if choice == Some(1) {
                        if let Ok(n) = trimmed.parse::<u8>() {
                            if (1..=10).contains(&n) {
                                match api::create_game(n, matchmaker_host) {
                                    Ok((response, addr)) => {
                                        match net::connect_token_from_base64(
                                            &response.connect_token,
                                        ) {
                                            Ok(token) => {
                                                if let Some(passcode) =
                                                    Passcode::from_string(&response.passcode)
                                                {
                                                    return Some((addr, token, passcode, true));
                                                }
                                            }
                                            Err(e) => {
                                                ui.show_sanitized_error(&e);
                                                await_error_dismissal(ui, font).await;
                                                return None;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        ui.show_sanitized_error(&e.to_string());
                                        await_error_dismissal(ui, font).await;
                                        return None;
                                    }
                                }
                            }
                        }
                        ui.show_error("Player count must be 1-10.");
                        prompt_printed = false;
                    } else {
                        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                            match api::join_game(trimmed, matchmaker_host) {
                                Ok((response, addr)) => {
                                    match net::connect_token_from_base64(&response.connect_token) {
                                        Ok(token) => {
                                            if let Some(passcode) = Passcode::from_string(trimmed) {
                                                return Some((addr, token, passcode, false));
                                            }
                                        }
                                        Err(e) => {
                                            ui.show_sanitized_error(&e);
                                            await_error_dismissal(ui, font).await;
                                            return None;
                                        }
                                    }
                                }
                                Err(_e) => {
                                    wrong_guesses += 1;
                                    if wrong_guesses >= JOIN_GUESS_LIMIT {
                                        ui.show_sanitized_error(
                                            "Wrong passcode. No such game found",
                                        );
                                        choice = None;
                                        choice_displayed = false;
                                        wrong_guesses = 0;
                                        prompt_printed = false;
                                    } else {
                                        let remaining = JOIN_GUESS_LIMIT - wrong_guesses;
                                        ui.show_sanitized_error(&format!(
                                            "Wrong passcode. {} {} remaining.",
                                            remaining,
                                            if remaining == 1 { "guess" } else { "guesses" }
                                        ));
                                        prompt_printed = false;
                                    }
                                }
                            }
                        } else {
                            ui.show_error("Passcode must be 6 digits.");
                            prompt_printed = false;
                        }
                    }
                }
                Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                    ui.show_sanitized_error(&format!("No connection: {}.", e));
                    await_error_dismissal(ui, font).await;
                    return None;
                }
                Ok(None) => {}
            }
        }

        if !prompt_printed {
            if choice.is_none() {
                ui.show_prompt("New game or Join game?");
                ui.show_message(" ");
                let menu_lines = format_new_join_menu_lines(selected_index);
                for (line, color) in &menu_lines {
                    ui.show_message_with_color(line, *color);
                }
            } else {
                if !choice_displayed {
                    let choice_label = NEW_JOIN_MENU_ITEMS[(choice.unwrap() - 1) as usize];
                    let menu_line_count = 1 + format_new_join_menu_lines(0).len();
                    ui.replace_last_messages(
                        menu_line_count,
                        vec![(format!("{}.", choice_label), Color::WHITE)],
                    );
                    choice_displayed = true;
                }
                if choice == Some(1) {
                    ui.show_prompt("How many players (1-10)? ");
                } else {
                    ui.show_prompt("Enter passcode (6 digits): ");
                }
            }
            prompt_printed = true;
        }

        if choice.is_none() && prompt_printed {
            let menu_lines = format_new_join_menu_lines(selected_index);
            ui.replace_last_messages(menu_lines.len(), menu_lines);
        }

        let should_show_input = choice.is_some();
        ui.draw(
            should_show_input,
            should_show_input,
            font,
            None::<crate::lobby::ui::LobbyTimerInfo>,
        );

        next_frame().await;
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
