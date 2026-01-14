pub mod gui;

use std::{fmt, net::SocketAddr};

use macroquad::prelude::Font;

use common::{
    input::{UiKey, sanitize},
    player::UsernameError,
};
pub use gui::Gui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiErrorKind {
    UsernameValidation(UsernameError),
    UsernameServerError,
    PasscodeFormat,
    DifficultyInvalidChoice,
    NetworkDisconnect,
    Deserialization,
    Other,
}

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

pub trait LobbyUi {
    fn show_message(&mut self, message: &str);
    fn show_error(&mut self, message: &str);
    fn show_warning(&mut self, message: &str);
    fn show_prompt(&mut self, prompt: &str);
    fn draw(&self, should_show_input: bool, show_cursor: bool, font: Option<&Font>);
    fn poll_input(&mut self, limit: usize, is_host: bool) -> Result<Option<String>, UiInputError>;
    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError>;
    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64);
    fn draw_countdown(&mut self, countdown_text: &str, font: Option<&Font>);
    fn flush_input(&mut self) {}
    fn show_banner_message(&mut self, message: &str) {
        self.show_message(&format!("  {}", message));
    }

    fn show_sanitized_message(&mut self, message: &str) {
        self.show_message(&sanitize(message));
    }

    fn show_sanitized_error(&mut self, message: &str) {
        self.show_error(&sanitize(message));
    }

    fn show_sanitized_prompt(&mut self, message: &str) {
        self.show_prompt(&sanitize(message));
    }

    fn show_sanitized_banner_message(&mut self, message: &str) {
        self.show_banner_message(&sanitize(message));
    }

    fn show_typed_error(&mut self, _kind: UiErrorKind, message: &str) {
        self.show_sanitized_error(message);
    }
}
