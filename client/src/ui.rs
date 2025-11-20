pub mod terminal;

use std::{fmt, net::SocketAddr};

use shared::input::{UiKey, sanitize};
pub use terminal::TerminalUi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiInputError {
    Disconnected,
}

impl fmt::Display for UiInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiInputError::Disconnected => write!(f, "input source disconnected"),
        }
    }
}

impl std::error::Error for UiInputError {}

pub trait ClientUi {
    fn show_message(&mut self, message: &str);
    fn show_error(&mut self, message: &str);
    fn show_prompt(&mut self, prompt: &str);
    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError>;
    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError>;
    fn show_status_line(&mut self, message: &str);
    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64);

    fn show_sanitized_message(&mut self, message: &str) {
        self.show_message(&sanitize(message));
    }

    fn show_sanitized_error(&mut self, message: &str) {
        self.show_error(&sanitize(message));
    }

    fn show_sanitized_prompt(&mut self, message: &str) {
        self.show_prompt(&sanitize(message));
    }

    fn show_sanitized_status_line(&mut self, message: &str) {
        self.show_status_line(&sanitize(message));
    }
}
