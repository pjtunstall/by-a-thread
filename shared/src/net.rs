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
