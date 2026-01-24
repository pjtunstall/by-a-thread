use std::{
    env, io,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use dotenvy;

use renet::{ChannelConfig, ConnectionConfig, SendType};
use socket2::{Domain, Socket, Type};

pub fn get_connectable_address() -> SocketAddr {
    dotenvy::dotenv().ok();

    let ip = env::var("IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "5000".to_string());

    let address_string = format!("{}:{}", ip, port);
    address_string.parse().expect("Invalid IP or Port format")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppChannel {
    ReliableOrdered,
    Unreliable,
    ServerTime,
}

impl From<AppChannel> for u8 {
    fn from(channel: AppChannel) -> Self {
        match channel {
            AppChannel::ReliableOrdered => 0,
            AppChannel::Unreliable => 1,
            AppChannel::ServerTime => 2,
        }
    }
}

pub fn connection_config() -> ConnectionConfig {
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

    let client_channels_config = vec![reliable_config.clone(), unreliable_config.clone()];
    let server_channels_config = vec![reliable_config, unreliable_config, time_sync_config];

    ConnectionConfig {
        client_channels_config,
        server_channels_config,
        ..Default::default()
    }
}

pub fn bind_socket(addr: SocketAddr) -> io::Result<UdpSocket> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::DGRAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    Ok(socket.into())
}
