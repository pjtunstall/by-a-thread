use std::{collections::VecDeque, net::SocketAddr};

use bincode::{config::standard, serde::encode_to_vec};
use macroquad::prelude::Font;

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::{DisconnectKind, NetworkHandle},
};
use common::{input::UiKey, net::AppChannel, protocol::ServerMessage};

#[derive(Default)]
pub struct MockUi {
    pub messages: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub error_kinds: Vec<UiErrorKind>,
    pub prompts: Vec<String>,
    pub inputs: VecDeque<Result<Option<String>, UiInputError>>,
    pub keys: VecDeque<Result<Option<UiKey>, UiInputError>>,
    pub countdown_draws: Vec<String>,
    pub local_player_color: Option<common::player::Color>,
}

impl MockUi {
    pub fn with_inputs<I>(inputs: I) -> Self
    where
        I: IntoIterator<Item = Result<Option<String>, UiInputError>>,
    {
        Self {
            inputs: inputs.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl MockUi {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            error_kinds: Vec::new(),
            prompts: Vec::new(),
            inputs: VecDeque::new(),
            keys: VecDeque::new(),
            countdown_draws: Vec::new(),
            local_player_color: None,
        }
    }
}

impl LobbyUi for MockUi {
    fn draw(&self, _should_show_input: bool, _show_cursor: bool, _font: Option<&Font>) {
        // No-op for tests.
    }
    fn show_message(&mut self, message: &str) {
        self.messages.push(message.to_string());
    }

    fn show_error(&mut self, message: &str) {
        self.errors.push(message.to_string());
    }

    fn show_warning(&mut self, message: &str) {
        self.warnings.push(message.to_string());
    }

    fn show_typed_error(&mut self, kind: UiErrorKind, message: &str) {
        self.error_kinds.push(kind);
        self.show_sanitized_error(message);
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.prompts.push(prompt.to_string());
    }

    fn set_local_player_color(&mut self, color: common::player::Color) {
        self.local_player_color = Some(color);
    }

    fn poll_input(&mut self, limit: usize, _is_host: bool) -> Result<Option<String>, UiInputError> {
        self.inputs.pop_front().unwrap_or(Ok(None)).map(|opt| {
            opt.map(|mut s| {
                if s.len() > limit {
                    while s.len() > limit {
                        s.pop();
                    }
                    s
                } else {
                    s
                }
            })
        })
    }

    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError> {
        self.keys.pop_front().unwrap_or(Ok(None))
    }

    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr) {
        self.messages.push(format!(
            "Client Banner: Protocol={}, Server={}",
            protocol_id, server_addr
        ));
    }

    fn draw_countdown(&mut self, countdown_text: &str, _font: Option<&Font>) {
        self.countdown_draws.push(countdown_text.to_string());
    }
}

pub struct MockNetwork {
    is_connected_val: bool,
    is_disconnected_val: bool,
    disconnect_reason_val: String,
    disconnect_kind_val: DisconnectKind,
    messages_to_receive: VecDeque<Vec<u8>>,
    pub sent_messages: VecDeque<(AppChannel, Vec<u8>)>,
    rtt: f64,
}

impl MockNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    fn set_connected(&mut self, connected: bool) {
        self.is_connected_val = connected;
    }

    pub fn set_disconnected(&mut self, disconnected: bool, reason: &str) {
        self.is_disconnected_val = disconnected;
        self.disconnect_reason_val = reason.to_string();
        self.disconnect_kind_val = DisconnectKind::Other(reason.to_string());
    }

    pub fn set_disconnect_kind(&mut self, kind: DisconnectKind) {
        self.disconnect_kind_val = kind;
    }

    pub fn queue_server_message(&mut self, message: ServerMessage) {
        let data = encode_to_vec(&message, standard()).expect("failed to serialize test message");
        self.messages_to_receive.push_back(data);
    }
}

impl Default for MockNetwork {
    fn default() -> Self {
        Self {
            is_connected_val: false,
            is_disconnected_val: false,
            disconnect_reason_val: String::new(),
            disconnect_kind_val: DisconnectKind::Other("no reason given".to_string()),
            messages_to_receive: VecDeque::new(),
            sent_messages: VecDeque::new(),
            rtt: 0.0,
        }
    }
}

impl NetworkHandle for MockNetwork {
    fn is_connected(&self) -> bool {
        self.is_connected_val
    }

    fn is_disconnected(&self) -> bool {
        self.is_disconnected_val
    }

    fn get_disconnect_reason(&self) -> String {
        self.disconnect_reason_val.clone()
    }

    fn disconnect_kind(&self) -> DisconnectKind {
        self.disconnect_kind_val.clone()
    }

    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>) {
        self.sent_messages.push_back((channel, message));
    }

    fn receive_message(&mut self, _channel: AppChannel) -> Option<Vec<u8>> {
        self.messages_to_receive.pop_front()
    }

    fn rtt(&self) -> f64 {
        self.rtt
    }
}
