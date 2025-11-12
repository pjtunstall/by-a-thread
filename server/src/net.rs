use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use renet::{RenetServer, ServerEvent};
use renet_netcode::{ServerAuthentication, ServerConfig};

use shared::{self, net::AppChannel};

pub enum ServerNetworkEvent {
    ClientConnected { client_id: u64 },
    ClientDisconnected { client_id: u64, reason: String },
}

pub trait ServerNetworkHandle {
    fn get_event(&mut self) -> Option<ServerNetworkEvent>;
    fn clients_id(&self) -> Vec<u64>;
    fn receive_message(&mut self, client_id: u64, channel: AppChannel) -> Option<Vec<u8>>;
    fn send_message(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>);
    fn broadcast_message(&mut self, channel: AppChannel, message: Vec<u8>);
    fn disconnect(&mut self, client_id: u64);
    fn broadcast_message_except(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>);
}

pub struct RenetServerNetworkHandle<'a> {
    pub server: &'a mut RenetServer,
}

impl ServerNetworkHandle for RenetServerNetworkHandle<'_> {
    fn get_event(&mut self) -> Option<ServerNetworkEvent> {
        self.server.get_event().map(|event| match event {
            ServerEvent::ClientConnected { client_id } => {
                ServerNetworkEvent::ClientConnected { client_id }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                ServerNetworkEvent::ClientDisconnected {
                    client_id,
                    reason: reason.to_string(),
                }
            }
        })
    }

    fn broadcast_message_except(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>) {
        self.server
            .broadcast_message_except(client_id, channel, message);
    }

    fn clients_id(&self) -> Vec<u64> {
        self.server.clients_id()
    }

    fn receive_message(&mut self, client_id: u64, channel: AppChannel) -> Option<Vec<u8>> {
        self.server
            .receive_message(client_id, channel)
            .map(|bytes| bytes.to_vec())
    }

    fn send_message(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>) {
        self.server.send_message(client_id, channel, message);
    }

    fn broadcast_message(&mut self, channel: AppChannel, message: Vec<u8>) {
        self.server.broadcast_message(channel, message);
    }

    fn disconnect(&mut self, client_id: u64) {
        self.server.disconnect(client_id);
    }
}

pub fn server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

pub fn bind_socket(addr: SocketAddr) -> UdpSocket {
    UdpSocket::bind(addr).expect("failed to bind socket")
}

pub fn build_server_config(
    current_time: Duration,
    protocol_id: u64,
    server_addr: SocketAddr,
    private_key: [u8; 32],
) -> ServerConfig {
    ServerConfig {
        current_time,
        max_clients: 10,
        protocol_id,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Secure { private_key },
    }
}
