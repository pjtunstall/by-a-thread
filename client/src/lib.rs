mod net;
mod state;
mod state_handlers;
mod ui;

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::state::{ClientSession, ClientState};
use crate::state_handlers::{AppChannel, NetworkHandle};
use crate::ui::{ClientUi, TerminalUi};
use shared;

pub fn run_client() {
    let mut ui = TerminalUi::new().expect("failed to initialize terminal UI");

    let private_key = shared::auth::private_key();
    let client_id = rand::random::<u64>();
    let server_addr = net::default_server_addr();
    let protocol_id = shared::protocol_version();
    let current_time = shared::current_time();
    let connect_token = net::create_connect_token(
        current_time,
        protocol_id,
        client_id,
        server_addr,
        &private_key,
    );

    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(e) => {
            ui.show_message(&format!("Failed to bind client socket: {}.", e));
            return;
        }
    };
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = match NetcodeClientTransport::new(current_time, authentication, socket) {
        Ok(transport) => transport,
        Err(e) => {
            ui.show_message(&format!("Failed to create network transport: {}", e));
            return;
        }
    };
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    ui.show_message(&format!(
        "Connecting to {} with client ID: {}.",
        server_addr, client_id
    ));

    let mut session = ClientSession::new();

    main_loop(&mut session, &mut ui, &mut client, &mut transport);

    ui.show_message("Client shutting down.");
}

struct RenetNetworkHandle<'a> {
    client: &'a mut RenetClient,
    transport: &'a NetcodeClientTransport,
}

impl NetworkHandle for RenetNetworkHandle<'_> {
    fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    fn is_disconnected(&self) -> bool {
        self.client.is_disconnected()
    }

    fn get_disconnect_reason(&self) -> String {
        self.client
            .disconnect_reason()
            .map(|reason| format!("Renet - {:?}", reason))
            .or_else(|| {
                self.transport
                    .disconnect_reason()
                    .map(|reason| format!("Transport - {:?}", reason))
            })
            .unwrap_or_else(|| "no reason given".to_string())
    }

    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>) {
        let renet_channel = match channel {
            AppChannel::ReliableOrdered => DefaultChannel::ReliableOrdered,
            AppChannel::Unreliable => DefaultChannel::Unreliable,
        };
        self.client.send_message(renet_channel, message);
    }

    fn receive_message(&mut self, channel: AppChannel) -> Option<Vec<u8>> {
        let renet_channel = match channel {
            AppChannel::ReliableOrdered => DefaultChannel::ReliableOrdered,
            AppChannel::Unreliable => DefaultChannel::Unreliable,
        };
        self.client
            .receive_message(renet_channel)
            .map(|bytes| bytes.to_vec())
    }
}

fn main_loop(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &mut NetcodeClientTransport,
) {
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Transport error: {}", e),
                },
            ) {
                break;
            }
            continue;
        }

        client.update(duration);

        // We create the handle inside a new scope.
        // This ensures its mutable borrows of `client` and `transport`
        // are released before `transport.send_packets` is called.
        let next_state = {
            let mut network_handle = RenetNetworkHandle { client, transport };
            match session.state() {
                ClientState::Startup { .. } => state_handlers::startup(session, ui),
                ClientState::Connecting => {
                    state_handlers::connecting(session, ui, &mut network_handle)
                }
                ClientState::Authenticating { .. } => {
                    state_handlers::authenticating(session, ui, &mut network_handle)
                }
                ClientState::ChoosingUsername { .. } => {
                    state_handlers::choosing_username(session, ui, &mut network_handle)
                }
                ClientState::InChat => state_handlers::in_chat(session, ui, &mut network_handle),
                ClientState::Countdown => {
                    state_handlers::countdown(session, ui, &mut network_handle)
                }
                ClientState::Disconnected { .. } => None,
            }
        };

        if let Some(new_state) = next_state {
            if apply_transition(session, ui, new_state) {
                break;
            }
            continue;
        }

        if let Err(e) = transport.send_packets(client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Error sending packets: {}", e),
                },
            ) {
                break;
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn apply_transition(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    new_state: ClientState,
) -> bool {
    session.transition(new_state);
    if let ClientState::Disconnected { message } = session.state() {
        ui.show_message(message);
        true
    } else {
        false
    }
}
