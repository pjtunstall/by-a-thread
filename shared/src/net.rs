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

pub fn private_key() -> [u8; 32] {
    [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ]
}
