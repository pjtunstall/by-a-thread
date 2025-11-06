mod network;
mod state;
mod transitions;
mod ui;

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use renet::{ConnectionConfig, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::state::{ClientSession, ClientState};
use crate::ui::{ClientUi, TerminalUi};

pub fn run_client() {
    let mut ui = TerminalUi::new();

    let private_key = network::client_private_key();
    let client_id = rand::random::<u64>();
    let server_addr = network::default_server_addr();
    let protocol_id = network::protocol_version();
    let current_time = network::current_time();
    let connect_token = network::create_connect_token(
        current_time,
        protocol_id,
        client_id,
        server_addr,
        &private_key,
    );

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket");
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("Failed to create transport");
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    ui.show_message(&format!(
        "Connecting to {} with client ID: {}",
        server_addr, client_id
    ));

    let mut session = ClientSession::new();

    main_loop(&mut session, &mut ui, &mut client, &mut transport);

    ui.show_message("Client shutting down");
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

        let next_state = match session.state() {
            ClientState::Startup { .. } => transitions::startup(session, ui),
            ClientState::Connecting => transitions::connecting(session, ui, client, transport),
            ClientState::Authenticating { .. } => {
                transitions::authenticating(session, ui, client, transport)
            }
            ClientState::ChoosingUsername { .. } => {
                transitions::choosing_username(session, ui, client, transport)
            }
            ClientState::InChat => transitions::in_chat(session, ui, client, transport),
            ClientState::Disconnected { .. } => None,
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
