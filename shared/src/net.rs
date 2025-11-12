use std::time::Duration;

use renet::{ChannelConfig, ConnectionConfig, SendType};

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
