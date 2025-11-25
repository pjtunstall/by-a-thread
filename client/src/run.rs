use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::*;
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    game,
    lobby::{
        self,
        ui::{Gui, LobbyUi},
    },
    net::{self, RenetNetworkHandle},
    resources::Resources,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use shared::{self, player::Player};

pub struct ClientRunner {
    pub session: ClientSession,
    pub client: RenetClient,
    pub transport: NetcodeClientTransport,
    pub ui: Gui,
    last_updated: Instant,
    resources: Resources,
}

impl ClientRunner {
    pub async fn new(
        socket: UdpSocket,
        server_addr: SocketAddr,
        private_key: [u8; 32],
        ui: Gui,
    ) -> Result<Self, String> {
        let resources = Resources::load().await;
        let client_id = ::rand::random::<u64>();
        let protocol_id = shared::protocol::version();
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
        let connection_config = shared::net::connection_config();
        let client = RenetClient::new(connection_config);
        let session = ClientSession::new(client_id);

        Ok(Self {
            session,
            client,
            transport,
            ui,
            last_updated: Instant::now(),
            resources,
        })
    }

    pub fn pump_network(&mut self) {
        if self.session.state().is_disconnected() {
            return;
        }

        let now = Instant::now();
        let duration = now - self.last_updated;
        self.last_updated = now;

        let mut result: Result<(), String> = Ok(());

        if let Err(e) = self.transport.update(duration, &mut self.client) {
            result = Err(format!("transport update failed: {}", e));
        }

        self.client.update(duration);

        {
            let mut network_handle = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
            crate::time::update_clock(&mut self.session, &mut network_handle, duration);
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
        match self.session.state() {
            ClientState::Game(_) => {
                if let Some(next_state) = game::handlers::update(&mut self.session, &self.resources)
                {
                    self.session.transition(next_state);
                }
            }
            ClientState::Disconnected { .. } => {
                self.ui.draw(false, false);
            }
            ClientState::Lobby(_) => {
                lobby::handlers::update(self).await;
            }
            _ => {
                todo!();
            }
        }
    }

    pub fn start_game(&mut self) -> Result<(), ()> {
        let old_state = std::mem::take(self.session.state_mut());
        match old_state {
            ClientState::Lobby(Lobby::Countdown { maze, players, .. }) => {
                self.session
                    .transition(ClientState::Game(game::Game { maze, players }));
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
        shared::protocol::version(),
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

pub fn print_player_list(
    ui: &mut dyn LobbyUi,
    session: &ClientSession,
    players: &HashMap<u64, Player>,
) {
    ui.show_message("\nPlayers:");
    for player in players.values() {
        let is_self = if player.id == session.client_id {
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
