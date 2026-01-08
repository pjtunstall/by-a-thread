use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    assets::Assets,
    game,
    lobby::{
        self,
        ui::{Gui, LobbyUi},
    },
    net::{self, DisconnectKind, RenetNetworkHandle},
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{self, player::Player};

#[derive(Debug)]
pub struct ClientRunner {
    pub session: ClientSession,
    pub client: RenetClient,
    pub transport: NetcodeClientTransport,
    pub ui: Gui,
    last_updated: Instant,
    last_frame_dt: Duration,
    assets: Assets,
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
            .map_err(|e| format!("Failed to set socket as non-blocking: {}", e))?;
        let connect_token = net::create_connect_token(
            current_time_duration,
            protocol_id,
            client_id,
            server_addr,
            &private_key,
        );
        let authentication = ClientAuthentication::Secure { connect_token };
        let transport = NetcodeClientTransport::new(current_time_duration, authentication, socket)
            .map_err(|e| format!("Failed to create network transport: {}", e))?;
        let connection_config = common::net::connection_config();
        let client = RenetClient::new(connection_config);
        let session = ClientSession::new(client_id);

        Ok(Self {
            session,
            client,
            transport,
            ui,
            last_updated: Instant::now(),
            last_frame_dt: Duration::ZERO,
            assets,
        })
    }

    pub fn pump_network(&mut self) {
        if self.session.state().is_disconnected() {
            return;
        }

        let now = Instant::now();
        let dt = now - self.last_updated;
        self.last_frame_dt = dt;
        // let unix_now = SystemTime::now()
        //     .duration_since(UNIX_EPOCH)
        //     .expect("time went backwards")
        //     .as_secs_f64();
        // println!(
        //     "pump. dt: {:.3}, unix_now: {:.3}",
        //     dt.as_secs_f64(),
        //     unix_now
        // );
        self.last_updated = now;

        let mut result: Result<(), String> = Ok(());

        if let Err(e) = self.transport.update(dt, &mut self.client) {
            result = Err(format!("transport update failed: {}", e));
        }

        self.client.update(dt);

        {
            let mut network_handle = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
            crate::time::update_clock(&mut self.session, &mut network_handle, dt);
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
            eprintln!("Disconnected: {}{}", disconnect_message, separator);
            self.session.disconnected_notified = true;
            self.ui.draw(false, false);
        }
    }

    async fn update_client_state(&mut self) {
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

        let smoothed_rtt = self.session.smoothed_rtt;
        let estimated_server_time = self.session.estimated_server_time;
        let mut accumulator = self.session.accumulator;
        let mut current_tick = self.session.current_tick;
        let mut simulated_time =
            current_tick as f64 * crate::time::TICK_DURATION_IDEAL + accumulator;
        let dt_seconds = self
            .last_frame_dt
            .as_secs_f64()
            // Clamp to avoid huge jumps if a frame stalls.
            .min(0.25);

        let mut should_transition = false;
        let mut next_state_option = None;

        match self.session.state_mut() {
            ClientState::Game(game_state) => {
                let mut network_handle =
                    RenetNetworkHandle::new(&mut self.client, &mut self.transport);
                let (updated_accumulator, updated_sim_time) = crate::time::update_accumulator(
                    accumulator,
                    simulated_time,
                    smoothed_rtt,
                    estimated_server_time,
                    dt_seconds,
                );
                accumulator = updated_accumulator;
                simulated_time = updated_sim_time;
                let target_tick =
                    crate::time::calculate_target_tick(smoothed_rtt, estimated_server_time);

                println!(
                    "target_tick - current_tick: {}, accumulator: {}, dt: {}",
                    target_tick - current_tick,
                    accumulator,
                    dt_seconds,
                );

                // A failsafe to prevent the accumulator from growing ever
                // greater if we fall behind.
                const MAX_TICKS_PER_FRAME: u8 = 8;
                let mut ticks_processed = 0;
                while accumulator >= crate::time::TICK_DURATION_IDEAL
                    && ticks_processed < MAX_TICKS_PER_FRAME
                {
                    if let Some(next_state) = game::handlers::handle(
                        game_state,
                        &self.assets,
                        &mut network_handle,
                        target_tick,
                    ) {
                        should_transition = true;
                        next_state_option = Some(next_state);
                        break;
                    }

                    accumulator -= crate::time::TICK_DURATION_IDEAL;
                    simulated_time += crate::time::TICK_DURATION_IDEAL;
                    current_tick += 1;
                    ticks_processed += 1;

                    // If we hit the limit, discard the remaining accumulator to prevent spiral.
                    if ticks_processed >= MAX_TICKS_PER_FRAME {
                        accumulator = 0.0;
                        println!("Death spiral detected: skipping ticks to catch up.");
                    }
                }

                let _alpha = accumulator / crate::time::TICK_DURATION_IDEAL;
                // render(alpha);

                self.session.accumulator = accumulator;
                self.session.current_tick = current_tick;

                println!("accumulator after processing: {}", accumulator);

                if should_transition {
                    self.session.transition(
                        next_state_option
                            .expect("should be a `next_state` `should_transition` from game"),
                    );
                }
            }
            // TODO: Following the pattern of the game handler, pass inner state to each
            // of the lobby substate handlers so as to let the type system enforce that the
            // correct type is sent, rather than having explicit guards at the start of each
            // handler. This will mean passing the inner state to the handler, rather than
            // passing `session`.`
            ClientState::Lobby(_) => {
                lobby::handlers::update(self).await;
            }
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
        self.session.current_tick =
            crate::time::calculate_initial_tick(self.session.estimated_server_time);
        self.last_updated = Instant::now();

        let initial_data = match self.session.state_mut() {
            ClientState::Lobby(Lobby::Countdown { game_data, .. }) => std::mem::take(game_data),
            other => {
                self.ui.show_sanitized_error(&format!(
                    "Tried to start game from invalid state: {:#?}.",
                    other
                ));
                return Err(());
            }
        };

        let Some(local_player_index) = initial_data
            .players
            .iter()
            .position(|p| p.client_id == self.session.client_id)
        else {
            self.session.transition(ClientState::Disconnected {
                message: format!("could not find you in list of players"),
            });
            return Err(());
        };

        self.session.local_player_index = Some(local_player_index);
        self.session
            .transition(ClientState::Game(game::state::Game::new(
                local_player_index,
                initial_data,
            )));

        Ok(())
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

        runner.pump_network();
        runner.update_client_state().await;

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
