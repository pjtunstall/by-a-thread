use std::{
    net::{SocketAddr, UdpSocket},
    time::{Instant, SystemTime, UNIX_EPOCH},
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
    net::{self, RenetNetworkHandle},
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
            assets,
        })
    }

    pub fn pump_network(&mut self) {
        if self.session.state().is_disconnected() {
            return;
        }

        let now = Instant::now();
        let dt = now - self.last_updated;
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
                self.ui
                    .show_sanitized_error(&format!("No connection: {}.", e));
                self.session.transition(ClientState::Disconnected {
                    message: format!("transport error: {}", e),
                });
            }
        }
    }

    async fn update_client_state(&mut self) {
        match self.session.state_mut() {
            ClientState::Game(game_state) => {
                let mut network_handle =
                    RenetNetworkHandle::new(&mut self.client, &mut self.transport);
                if let Some(next_state) =
                    game::handlers::handle(game_state, &self.assets, &mut network_handle)
                {
                    self.session.transition(next_state);
                }
            }
            ClientState::Disconnected { .. } => {
                self.ui.draw(false, false);
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
    }

    pub fn start_game(&mut self) -> Result<(), ()> {
        let old_state = std::mem::take(self.session.state_mut());
        match old_state {
            ClientState::Lobby(Lobby::Countdown { snapshot, .. }) => {
                self.session
                    .transition(ClientState::Game(game::state::Game::new(
                        self.session.local_player_index,
                        snapshot,
                    )));

                if let ClientState::Game(game) = self.session.state() {
                    self.session.local_player_index = game
                        .snapshot
                        .players
                        .iter()
                        .position(|p| p.client_id == self.session.client_id)
                        .expect("current player should be in game players list");
                }

                Ok(())
            }
            other => {
                self.ui.show_sanitized_error(&format!(
                    "Tried to start game from invalid state: {:#?}.",
                    other
                ));
                Err(())
            }
        }
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
            player.color.as_str(),
            is_self
        ));
    }
    ui.show_sanitized_message("");
}
