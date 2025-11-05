use super::{ClientUi, UiInputError};
use macroquad::prelude::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

const MAX_LOG_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiLogLevel {
    Message,
    Error,
}

#[derive(Debug, Clone)]
pub struct UiLogEntry {
    pub text: String,
    pub level: UiLogLevel,
}

impl UiLogEntry {
    fn message<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            level: UiLogLevel::Message,
        }
    }

    fn error<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            level: UiLogLevel::Error,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct UiState {
    pub log: Vec<UiLogEntry>,
    pub prompt: Option<String>,
}

impl UiState {
    fn push_entry(&mut self, entry: UiLogEntry) {
        self.log.push(entry);
        if self.log.len() > MAX_LOG_LINES {
            let overflow = self.log.len() - MAX_LOG_LINES;
            self.log.drain(0..overflow);
        }
    }

    fn set_prompt<S: Into<String>>(&mut self, prompt: S) {
        self.prompt = Some(prompt.into());
    }
}

pub struct MacroquadUi {
    state: Arc<Mutex<UiState>>,
    rx: Receiver<String>,
    tx: Sender<String>,
}

impl MacroquadUi {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(UiState::default()));
        let (tx, rx) = mpsc::channel();
        Self { state, rx, tx }
    }

    pub fn state_handle(&self) -> Arc<Mutex<UiState>> {
        Arc::clone(&self.state)
    }

    pub fn input_sender(&self) -> Sender<String> {
        self.tx.clone()
    }
}

impl ClientUi for MacroquadUi {
    fn show_message(&mut self, message: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.push_entry(UiLogEntry::message(message));
        }
    }

    fn show_error(&mut self, message: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.push_entry(UiLogEntry::error(message));
        }
    }

    fn show_prompt(&mut self, prompt: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.set_prompt(prompt);
        }
    }

    fn poll_input(&mut self) -> Result<Option<String>, UiInputError> {
        match self.rx.try_recv() {
            Ok(input) => Ok(Some(input)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(UiInputError::Disconnected),
        }
    }
}

pub async fn run_macroquad_ui(state: Arc<Mutex<UiState>>, tx: Sender<String>) {
    let mut input_buffer = String::new();

    loop {
        clear_background(BLACK);

        while let Some(ch) = get_char_pressed() {
            if !ch.is_control() {
                input_buffer.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            input_buffer.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            let submission = input_buffer.trim().to_string();
            if !submission.is_empty() {
                let _ = tx.send(submission);
            }
            input_buffer.clear();
        }

        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        let (log_entries, prompt) = match state.lock() {
            Ok(guard) => (guard.log.clone(), guard.prompt.clone()),
            Err(_) => (Vec::new(), None),
        };

        let mut y = 30.0;
        let margin = 20.0;
        let max_width = screen_width() - margin * 2.0;

        for entry in log_entries.iter().rev().take(18).rev() {
            let color = match entry.level {
                UiLogLevel::Message => WHITE,
                UiLogLevel::Error => RED,
            };

            draw_text_ex(
                &entry.text,
                margin,
                y,
                TextParams {
                    font_size: 20,
                    color,
                    ..Default::default()
                },
            );
            y += 24.0;
        }

        if let Some(prompt_text) = prompt {
            draw_text_ex(
                &prompt_text,
                margin,
                screen_height() - 80.0,
                TextParams {
                    font_size: 22,
                    color: YELLOW,
                    ..Default::default()
                },
            );
        }

        let input_display = format!("> {}", input_buffer);
        draw_rectangle(
            margin - 5.0,
            screen_height() - 50.0,
            max_width + 10.0,
            36.0,
            Color::from_rgba(30, 30, 30, 220),
        );
        draw_text_ex(
            &input_display,
            margin,
            screen_height() - 25.0,
            TextParams {
                font_size: 22,
                color: WHITE,
                ..Default::default()
            },
        );

        next_frame().await;
    }
}
