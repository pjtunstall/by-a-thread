use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use renet::RenetClient;
use renet_netcode::{ConnectToken, NetcodeClientTransport, NetcodeDisconnectReason};

use common::net::AppChannel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectKind {
    DisconnectedByServer,
    DisconnectedByClient,
    Transport,
    TokenExpired,
    ConnectionTimedOut,
    ConnectionDenied,
    Other(String),
}

pub fn default_server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

pub fn create_connect_token(
    current_time: Duration,
    protocol_id: u64,
    client_id: u64,
    server_addr: SocketAddr,
    private_key: &[u8; 32],
) -> ConnectToken {
    ConnectToken::generate(
        current_time,
        protocol_id,
        3600,
        client_id,
        15,
        vec![server_addr],
        None,
        private_key,
    )
    .expect("failed to generate token")
}

pub struct RenetNetworkHandle<'a> {
    client: &'a mut RenetClient,
    transport: &'a NetcodeClientTransport,
}

impl<'a> RenetNetworkHandle<'a> {
    pub fn new(client: &'a mut RenetClient, transport: &'a mut NetcodeClientTransport) -> Self {
        Self { client, transport }
    }
}

pub trait NetworkHandle {
    fn is_connected(&self) -> bool;
    fn is_disconnected(&self) -> bool;
    fn get_disconnect_reason(&self) -> String;
    fn disconnect_kind(&self) -> DisconnectKind;
    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>);
    fn receive_message(&mut self, channel: AppChannel) -> Option<Vec<u8>>;
    fn rtt(&self) -> f64;
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

    fn disconnect_kind(&self) -> DisconnectKind {
        map_disconnect_kind(
            self.client.disconnect_reason(),
            self.transport.disconnect_reason(),
        )
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

pub fn map_disconnect_kind(
    renet_reason: Option<renet::DisconnectReason>,
    transport_reason: Option<NetcodeDisconnectReason>,
) -> DisconnectKind {
    if let Some(reason) = renet_reason {
        match reason {
            renet::DisconnectReason::DisconnectedByServer => {
                return DisconnectKind::DisconnectedByServer;
            }
            renet::DisconnectReason::DisconnectedByClient => {
                return DisconnectKind::DisconnectedByClient;
            }
            renet::DisconnectReason::Transport => {
                if let Some(kind) = map_transport_reason(&transport_reason) {
                    return kind;
                }
                return DisconnectKind::Transport;
            }
            renet::DisconnectReason::PacketSerialization(_)
            | renet::DisconnectReason::PacketDeserialization(_)
            | renet::DisconnectReason::ReceivedInvalidChannelId(_)
            | renet::DisconnectReason::SendChannelError { .. }
            | renet::DisconnectReason::ReceiveChannelError { .. } => {
                return DisconnectKind::Other(reason.to_string());
            }
        }
    }

    if let Some(kind) = map_transport_reason(&transport_reason) {
        return kind;
    }

    DisconnectKind::Other("no reason given".to_string())
}

fn map_transport_reason(reason: &Option<NetcodeDisconnectReason>) -> Option<DisconnectKind> {
    let Some(reason) = reason else { return None };
    Some(match reason {
        NetcodeDisconnectReason::ConnectionDenied
        | NetcodeDisconnectReason::DisconnectedByServer => DisconnectKind::DisconnectedByServer,
        NetcodeDisconnectReason::DisconnectedByClient => DisconnectKind::DisconnectedByClient,
        NetcodeDisconnectReason::ConnectionTimedOut
        | NetcodeDisconnectReason::ConnectionResponseTimedOut
        | NetcodeDisconnectReason::ConnectionRequestTimedOut => DisconnectKind::ConnectionTimedOut,
        NetcodeDisconnectReason::ConnectTokenExpired => DisconnectKind::TokenExpired,
    })
}
