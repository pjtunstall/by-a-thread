mod net;
mod state;
mod state_handlers;
mod ui;

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use renet::{ChannelConfig, ConnectionConfig, RenetClient, SendType};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::state::{ClientSession, ClientState};
use crate::state_handlers::{AppChannel, NetworkHandle};
use crate::ui::{ClientUi, TerminalUi};
use shared;
use shared::ServerMessage;

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

    let reliable_config = ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(100),
        },
    };
    let unreliable_config = ChannelConfig {
        channel_id: 1,
        max_memory_usage_bytes: 10 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };
    let time_sync_config = ChannelConfig {
        channel_id: 2,
        max_memory_usage_bytes: 1 * 1024 * 1024,
        send_type: SendType::Unreliable,
    };

    let client_channels_config = vec![
        reliable_config.clone(),
        unreliable_config.clone(),
        time_sync_config.clone(),
    ];
    let server_channels_config = vec![reliable_config, unreliable_config, time_sync_config];

    let connection_config = ConnectionConfig {
        client_channels_config,
        server_channels_config,
        ..Default::default()
    };

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

    fn rtt(&self) -> f64 {
        self.client.rtt()
    }

    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>) {
        self.client.send_message(channel, message);
    }

    fn receive_message(&mut self, channel: AppChannel) -> Option<Vec<u8>> {
        self.client
            .receive_message(channel)
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

        let mut network_handle = RenetNetworkHandle { client, transport };

        let next_state_from_messages = process_network_messages(session, &mut network_handle);

        let next_state_from_logic = match session.state() {
            ClientState::Startup { .. } => state_handlers::startup(session, ui),
            ClientState::Connecting => state_handlers::connecting(session, ui, &mut network_handle),
            ClientState::Authenticating { .. } => {
                state_handlers::authenticating(session, ui, &mut network_handle)
            }
            ClientState::ChoosingUsername { .. } => {
                state_handlers::choosing_username(session, ui, &mut network_handle)
            }
            ClientState::InChat => state_handlers::in_chat(session, ui, &mut network_handle),
            ClientState::Countdown => state_handlers::countdown(session, ui),
            ClientState::Disconnected { .. } => None,
        };

        if let Some(new_state) = next_state_from_messages.or(next_state_from_logic) {
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

fn process_network_messages(
    session: &mut ClientSession,
    network: &mut RenetNetworkHandle,
) -> Option<ClientState> {
    let mut next_state = None;

    while let Some(message) = network.receive_message(AppChannel::ReliableOrdered) {
        match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
            Ok((ServerMessage::CountdownStarted { end_time }, _)) => {
                session.countdown_end_time = Some(end_time);
                next_state = Some(ClientState::Countdown);
            }
            _ => {}
        }
    }

    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
            Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
                let rtt = network.rtt();
                let one_way_latency = (rtt / 1000.0) / 2.0;
                session.estimated_server_time = server_sent_time + one_way_latency;
            }
            _ => {}
        }
    }

    next_state
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
