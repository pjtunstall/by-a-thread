use renet::DefaultChannel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppChannel {
    ReliableOrdered,
    Unreliable,
}

impl From<AppChannel> for DefaultChannel {
    fn from(channel: AppChannel) -> Self {
        match channel {
            AppChannel::ReliableOrdered => DefaultChannel::ReliableOrdered,
            AppChannel::Unreliable => DefaultChannel::Unreliable,
        }
    }
}
