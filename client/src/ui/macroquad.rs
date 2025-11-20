use std::net::SocketAddr;

use macroquad::prelude::*;

use crate::ui::{ClientUi, UiInputError};
use shared::input::UiKey;

const FONT_SIZE: f32 = 24.0;
const TEXT_COLOR: Color = WHITE;
const ERROR_COLOR: Color = RED;
const PROMPT_COLOR: Color = LIGHTGRAY;
const BANNER_COLOR: Color = YELLOW;
const BACKGROUND_COLOR: Color = BLACK;

pub struct MacroquadUi {
    input_buffer: String,
    pub message_history: Vec<(String, Color)>,
    status_message: String,
    max_history_lines: usize,
    cursor_pos: usize,
}

impl MacroquadUi {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            message_history: Vec::new(),
            status_message: String::new(),
            max_history_lines: 20,
            cursor_pos: 0,
        }
    }

    fn add_history(&mut self, message: &str, color: Color) {
        let max_chars_per_line = (screen_width() / FONT_SIZE * 1.5) as usize;
        let mut wrapped_message = String::new();
        for line in message.lines() {
            if line.is_empty() {
                wrapped_message.push('\n');
                continue;
            }
            for chunk in line.as_bytes().chunks(max_chars_per_line) {
                wrapped_message.push_str(std::str::from_utf8(chunk).unwrap_or("..."));
                wrapped_message.push('\n');
            }
        }

        let trimmed_message = wrapped_message.trim().to_string();

        self.message_history.push((trimmed_message, color));
        if self.message_history.len() > self.max_history_lines {
            self.message_history.remove(0);
        }
    }

    pub fn draw(&self) {
        clear_background(BACKGROUND_COLOR);

        let line_height = FONT_SIZE * 1.2;
        let prompt_y = screen_height() - line_height;
        let mut y = prompt_y - line_height;

        draw_text(&self.status_message, 10.0, 30.0, FONT_SIZE, ERROR_COLOR);

        for (message, color) in self.message_history.iter().rev() {
            let lines: Vec<&str> = message.lines().collect();
            for line in lines.iter().rev() {
                if y < line_height * 2.0 {
                    break;
                }
                draw_text(line, 10.0, y, FONT_SIZE, *color);
                y -= line_height;
            }
        }

        let prompt_text = format!("> {}", self.input_buffer);
        draw_text(&prompt_text, 10.0, prompt_y, FONT_SIZE, PROMPT_COLOR);
    }
}

impl ClientUi for MacroquadUi {
    fn show_message(&mut self, message: &str) {
        self.add_history(message, TEXT_COLOR);
    }

    fn show_error(&mut self, message: &str) {
        self.add_history(&format!("[ERROR] {}", message), ERROR_COLOR);
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.add_history(prompt, PROMPT_COLOR);
    }

    fn show_status_line(&mut self, message: &str) {
        self.status_message = message.to_string();
    }

    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64) {
        self.add_history(&format!("  Game version:  {}", protocol_id), BANNER_COLOR);
        self.add_history(&format!("  Connecting to: {}", server_addr), BANNER_COLOR);
        self.add_history(&format!("  Your ID:       {}", client_id), BANNER_COLOR);
    }

    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError> {
        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
            return Err(UiInputError::Disconnected);
        }

        if is_key_pressed(KeyCode::Tab) {
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return Ok(Some("\t".to_string()));
        }

        while let Some(char_code) = get_char_pressed() {
            let c = char_code as char;
            let at_limit = self.input_buffer.len() >= limit;

            if !at_limit && !c.is_control() {
                self.input_buffer.push(c);
                self.cursor_pos = self.input_buffer.len();
            }
        }

        if is_key_pressed(KeyCode::Enter) {
            let line = self.input_buffer.drain(..).collect();
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return Ok(Some(line));
        }

        if is_key_pressed(KeyCode::Backspace) {
            self.input_buffer.pop();
            self.cursor_pos = self.input_buffer.len();
        }

        if is_key_pressed(KeyCode::Escape) {
            self.input_buffer.clear();
            self.cursor_pos = 0;
        }

        Ok(None)
    }

    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError> {
        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
            return Err(UiInputError::Disconnected);
        }

        if let Some(key_code) = get_last_key_pressed() {
            let ui_key = match key_code {
                KeyCode::Enter => Some(UiKey::Enter),
                KeyCode::Backspace => Some(UiKey::Backspace),
                KeyCode::Escape => Some(UiKey::Esc),
                KeyCode::Tab => Some(UiKey::Tab),
                _ => {
                    if let Some(char_code) = get_char_pressed() {
                        Some(UiKey::Char(char_code as char))
                    } else {
                        None
                    }
                }
            };
            return Ok(ui_key);
        }

        Ok(None)
    }
}
