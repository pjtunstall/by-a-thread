use std::{
    fmt,
    io::{self, Stdout, Write, stdout},
    net::SocketAddr,
    time::Duration,
};

use crossterm::{
    cursor::{MoveToColumn, Show},
    event::{self, Event, KeyCode, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};

use shared::input::{UiKey, sanitize};

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

pub struct TerminalUi<W: Write> {
    stdout: W,
    buffer: String,
    cursor_pos: usize,
    prompt_lines: u16,
    cols: u16,
    is_raw_mode_owner: bool,
}

impl TerminalUi<Stdout> {
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = stdout();
        let (cols, _) = terminal::size().unwrap_or((80, 24));
        let cols = cols.max(1);
        execute!(
            stdout,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print("> ")
        )?;
        stdout.flush()?;

        execute!(stdout, Show).ok();

        Ok(Self {
            stdout,
            buffer: String::new(),
            cursor_pos: 0,
            prompt_lines: 1,
            cols,
            is_raw_mode_owner: true,
        })
    }
}

impl<W: Write> TerminalUi<W> {
    fn clear_prompt(&mut self) -> Result<(), UiInputError> {
        let map_err = |_| UiInputError::Disconnected;
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .map_err(map_err)?;
        }
        for i in 0..self.prompt_lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine)).map_err(map_err)?;
            if i + 1 < self.prompt_lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1)).map_err(map_err)?;
            }
        }
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .map_err(map_err)?;
        }
        queue!(self.stdout, MoveToColumn(0)).map_err(map_err)?;
        Ok(())
    }

    fn redraw_prompt(&mut self) -> Result<(), UiInputError> {
        let map_err = |_| UiInputError::Disconnected;
        let cols = usize::from(self.cols.max(1));
        let prompt = "> ";

        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .map_err(map_err)?;
        }

        queue!(
            self.stdout,
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown)
        )
        .map_err(map_err)?;

        let full = format!("{}{}", prompt, &self.buffer);
        let new_lines = (full.len().max(1) + cols - 1) / cols;
        self.prompt_lines = new_lines as u16;

        queue!(self.stdout, Print(prompt), Print(&self.buffer)).map_err(map_err)?;

        let prompt_len = prompt.len();
        let cursor_byte_pos = prompt_len + self.cursor_pos;
        let full_len = prompt_len + self.buffer.len();

        let cursor_row = cursor_byte_pos / cols;
        let end_row = full_len / cols;

        let cursor_col = (cursor_byte_pos % cols) as u16;
        let move_up = (end_row - cursor_row) as u16;

        if move_up > 0 {
            queue!(self.stdout, crossterm::cursor::MoveUp(move_up)).map_err(map_err)?;
        }
        queue!(self.stdout, MoveToColumn(cursor_col)).map_err(map_err)?;

        self.stdout.flush().map_err(map_err)?;
        stdout().flush().map_err(map_err)?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event, limit: usize) -> Result<Option<String>, UiInputError> {
        match event {
            Event::Key(key_event) => {
                if key_event.modifiers == KeyModifiers::CONTROL {
                    match key_event.code {
                        KeyCode::Char('c') | KeyCode::Char('d') => {
                            return Err(UiInputError::Disconnected);
                        }
                        _ => return Ok(None),
                    }
                }

                match key_event.code {
                    KeyCode::Enter => {
                        let line = self.buffer.drain(..).collect();
                        self.cursor_pos = 0;
                        self.clear_prompt()?;
                        self.prompt_lines = 1;
                        self.redraw_prompt()?;
                        Ok(Some(line))
                    }
                    KeyCode::Backspace => {
                        if self.cursor_pos > 0 {
                            let mut char_boundary = self.cursor_pos - 1;
                            while !self.buffer.is_char_boundary(char_boundary) {
                                char_boundary -= 1;
                            }
                            self.buffer.remove(char_boundary);
                            self.cursor_pos = char_boundary;
                            self.redraw_prompt()?;
                        }
                        Ok(None)
                    }
                    KeyCode::Esc => {
                        if !self.buffer.is_empty() {
                            self.buffer.clear();
                            self.cursor_pos = 0;
                            self.redraw_prompt()?;
                        }
                        Ok(None)
                    }
                    KeyCode::Tab => Ok(Some(String::from(shared::auth::START_COUNTDOWN))),
                    KeyCode::Char(c) => {
                        let at_limit = self.buffer.len() >= limit;
                        if !at_limit && !c.is_control() {
                            self.buffer.insert(self.cursor_pos, c);
                            self.cursor_pos += c.len_utf8();
                            self.redraw_prompt()?;
                        }
                        Ok(None)
                    }
                    KeyCode::Left => {
                        if self.cursor_pos > 0 {
                            let mut char_boundary = self.cursor_pos - 1;
                            while !self.buffer.is_char_boundary(char_boundary) {
                                char_boundary -= 1;
                            }
                            self.cursor_pos = char_boundary;
                            self.redraw_prompt()?;
                        }
                        Ok(None)
                    }
                    KeyCode::Right => {
                        if self.cursor_pos < self.buffer.len() {
                            let mut char_boundary = self.cursor_pos + 1;
                            while char_boundary < self.buffer.len()
                                && !self.buffer.is_char_boundary(char_boundary)
                            {
                                char_boundary += 1;
                            }
                            self.cursor_pos = char_boundary;
                            self.redraw_prompt()?;
                        }
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            Event::Resize(cols, _) => {
                self.cols = cols.max(1);
                self.redraw_prompt()?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

impl<W: Write> ClientUi for TerminalUi<W> {
    fn show_message(&mut self, message: &str) {
        if self.clear_prompt().is_err() {
            return;
        }
        queue!(self.stdout, Print(message), Print("\r\n")).ok();
        self.prompt_lines = 1;
        self.redraw_prompt().ok();
    }

    fn show_error(&mut self, message: &str) {
        if self.clear_prompt().is_err() {
            return;
        }
        queue!(
            self.stdout,
            MoveToColumn(0),
            Print("[ERROR] "),
            Print(message),
            Print("\r\n"),
        )
        .ok();

        self.prompt_lines = 1;
        self.stdout.flush().ok();
        self.redraw_prompt().ok();
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.show_message(prompt);
    }

    fn show_status_line(&mut self, message: &str) {
        if self.clear_prompt().is_err() {
            return;
        }
        queue!(self.stdout, Print(message), Clear(ClearType::UntilNewLine)).ok();
        self.prompt_lines = 1;
        self.stdout.flush().ok();
    }

    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError> {
        let has_event = event::poll(Duration::ZERO).map_err(|_| UiInputError::Disconnected)?;

        if !has_event {
            return Ok(None);
        }

        let event = event::read().map_err(|_| UiInputError::Disconnected)?;

        self.handle_event(event, limit)
    }

    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError> {
        let has_event = event::poll(Duration::ZERO).map_err(|_| UiInputError::Disconnected)?;

        if !has_event {
            return Ok(None);
        }

        let event = event::read().map_err(|_| UiInputError::Disconnected)?;

        match event {
            Event::Key(key_event) => {
                if key_event.modifiers == KeyModifiers::CONTROL {
                    match key_event.code {
                        KeyCode::Char('c') | KeyCode::Char('d') => {
                            return Err(UiInputError::Disconnected);
                        }
                        _ => return Ok(None),
                    }
                }

                let ui_key = match key_event.code {
                    KeyCode::Char(c) => Some(UiKey::Char(c)),
                    KeyCode::Enter => Some(UiKey::Enter),
                    KeyCode::Backspace => Some(UiKey::Backspace),
                    KeyCode::Esc => Some(UiKey::Esc),
                    KeyCode::Tab => Some(UiKey::Tab),
                    _ => None,
                };
                Ok(ui_key)
            }
            Event::Resize(cols, _) => {
                self.cols = cols.max(1);
                self.redraw_prompt()
                    .map_err(|_| UiInputError::Disconnected)?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64) {
        self.show_message(&format!("  Game version:  {}", protocol_id));
        self.show_message(&format!("  Connecting to: {}", server_addr));
        self.show_message(&format!("  Your ID:       {}", client_id));
    }
}

impl<W: Write> Drop for TerminalUi<W> {
    fn drop(&mut self) {
        if self.is_raw_mode_owner {
            execute!(self.stdout, Print("\r\n"), Show).ok();
            terminal::disable_raw_mode().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use shared::chat::MAX_CHAT_MESSAGE_BYTES;

    fn setup_test_ui(cols: u16) -> TerminalUi<Vec<u8>> {
        TerminalUi {
            stdout: Vec::new(),
            buffer: String::new(),
            cursor_pos: 0,
            prompt_lines: 1,
            cols,
            is_raw_mode_owner: false,
        }
    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn test_simple_char_input() {
        let mut ui = setup_test_ui(80);

        ui.handle_event(key_event(KeyCode::Char('a')), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        assert_eq!(ui.buffer, "a");
        assert_eq!(ui.prompt_lines, 1);

        ui.handle_event(key_event(KeyCode::Char('b')), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        assert_eq!(ui.buffer, "ab");
        assert_eq!(ui.prompt_lines, 1);
    }

    #[test]
    fn test_simple_backspace() {
        let mut ui = setup_test_ui(80);
        ui.handle_event(key_event(KeyCode::Char('a')), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        ui.handle_event(key_event(KeyCode::Char('b')), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        assert_eq!(ui.buffer, "ab");

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        assert_eq!(ui.buffer, "a");
        assert_eq!(ui.prompt_lines, 1);

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .unwrap();
        assert_eq!(ui.buffer, "");
        assert_eq!(ui.prompt_lines, 1);

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("Backspace on empty buffer failed");
        assert_eq!(ui.buffer, "");
        assert_eq!(ui.prompt_lines, 1);
    }

    #[test]
    fn test_enter_key_returns_and_clears_buffer() {
        let mut ui = setup_test_ui(80);
        ui.handle_event(key_event(KeyCode::Char('h')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle 'h' key");
        ui.handle_event(key_event(KeyCode::Char('i')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle 'i' key");
        assert_eq!(ui.buffer, "hi");

        let result = ui
            .handle_event(key_event(KeyCode::Enter), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle Enter key");

        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hi");

        assert_eq!(ui.buffer, "");
        assert_eq!(ui.prompt_lines, 1);
    }

    #[test]
    fn test_multiline_backspace_unwraps_correctly() {
        let mut ui = setup_test_ui(10);

        for c in "12345678".chars() {
            ui.handle_event(key_event(KeyCode::Char(c)), MAX_CHAT_MESSAGE_BYTES)
                .expect("failed to handle char");
        }
        assert_eq!(ui.buffer, "12345678");
        assert_eq!(ui.prompt_lines, 1);

        ui.handle_event(key_event(KeyCode::Char('9')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle char");
        assert_eq!(ui.buffer, "123456789");
        assert_eq!(ui.prompt_lines, 2);

        ui.handle_event(key_event(KeyCode::Char('0')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle char");
        assert_eq!(ui.buffer, "1234567890");
        assert_eq!(ui.prompt_lines, 2);

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");
        assert_eq!(ui.buffer, "123456789");
        assert_eq!(ui.prompt_lines, 2);

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");

        assert_eq!(ui.buffer, "12345678");

        assert_eq!(ui.prompt_lines, 1);

        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");
        assert_eq!(ui.buffer, "1234567");
        assert_eq!(ui.prompt_lines, 1);
    }

    #[test]
    fn test_input_sanitization_filters_control_chars() {
        let mut ui = setup_test_ui(80);
        let limit = MAX_CHAT_MESSAGE_BYTES;

        ui.handle_event(key_event(KeyCode::Char('a')), limit)
            .unwrap();
        assert_eq!(ui.buffer, "a");

        let bell_char = '\x07';
        assert!(
            bell_char.is_control(),
            "Test setup error: Bell char should be a control character"
        );
        ui.handle_event(key_event(KeyCode::Char(bell_char)), limit)
            .unwrap();

        assert_eq!(
            ui.buffer, "a",
            "Buffer should not accept control characters"
        );

        ui.handle_event(key_event(KeyCode::Char('b')), limit)
            .unwrap();
        assert_eq!(ui.buffer, "ab");
    }
}
