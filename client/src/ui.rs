use std::fmt;
use std::io::{self, Stdout, Write, stdout};
use std::net::SocketAddr;
use std::time::Duration;

use crossterm::{
    cursor::MoveToColumn,
    event::{self, Event, KeyCode, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiInputError {
    Disconnected,
}

impl fmt::Display for UiInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiInputError::Disconnected => write!(f, "Input source disconnected"),
        }
    }
}

pub trait ClientUi {
    fn show_message(&mut self, message: &str);
    fn show_error(&mut self, message: &str);
    fn show_prompt(&mut self, prompt: &str);
    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError>;
    fn show_status_line(&mut self, message: &str);
    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64);
}

pub struct TerminalUi<W: Write> {
    stdout: W,
    buffer: String,
    prompt_lines: u16,
    cols: u16,
    is_raw_mode_owner: bool, // True except in tests.
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
        Ok(Self {
            stdout,
            buffer: String::new(),
            prompt_lines: 1,
            cols,
            is_raw_mode_owner: true,
        })
    }
}

impl<W: Write> TerminalUi<W> {
    fn clear_prompt(&mut self) -> io::Result<()> {
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )?;
        }
        for i in 0..self.prompt_lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            if i + 1 < self.prompt_lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1))?;
            }
        }
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )?;
        }
        queue!(self.stdout, MoveToColumn(0))
    }

    fn redraw_prompt(&mut self) -> io::Result<()> {
        let cols = usize::from(self.cols.max(1));
        let prompt = "> ";

        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )?;
        }

        queue!(
            self.stdout,
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown)
        )?;

        let full = format!("{}{}", prompt, &self.buffer);
        let new_lines = (full.len().max(1) + cols - 1) / cols;
        self.prompt_lines = new_lines as u16;

        queue!(self.stdout, Print(prompt), Print(&self.buffer))?;
        self.stdout.flush()?;
        stdout().flush()
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
                        queue!(self.stdout, Print("\r\n")).unwrap();
                        self.prompt_lines = 1;
                        self.redraw_prompt().unwrap();
                        Ok(Some(line))
                    }
                    KeyCode::Backspace => {
                        if self.buffer.pop().is_some() {
                            self.redraw_prompt().unwrap();
                        }
                        Ok(None)
                    }
                    KeyCode::Esc => {
                        if !self.buffer.is_empty() {
                            self.buffer.clear();
                            self.redraw_prompt().unwrap();
                        }
                        Ok(None)
                    }
                    KeyCode::Tab => Ok(Some(String::from(shared::auth::START_COUNTDOWN))),
                    KeyCode::Char(c) => {
                        let at_limit = self.buffer.len() >= limit;

                        if at_limit {
                            Ok(None)
                        } else {
                            self.buffer.push(c);
                            let cols = usize::from(self.cols.max(1));
                            let prompt = "> ";
                            let full = format!("{}{}", prompt, &self.buffer);
                            let new_lines = (full.len().max(1) + cols - 1) / cols;
                            self.prompt_lines = new_lines as u16;
                            queue!(self.stdout, Print(c)).expect("failed to write char");
                            self.stdout.flush().expect("failed to flush stdout");
                            stdout().flush().expect("failed to flush global stdout");
                            Ok(None)
                        }
                    }
                    _ => Ok(None),
                }
            }
            Event::Resize(cols, _) => {
                self.cols = cols.max(1);
                self.redraw_prompt().unwrap();
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

impl<W: Write> ClientUi for TerminalUi<W> {
    fn show_message(&mut self, message: &str) {
        self.clear_prompt().expect("failed to clear prompt");
        queue!(self.stdout, Print(message), Print("\r\n"),).expect("failed to show message");

        self.prompt_lines = 1;
        self.redraw_prompt().expect("failed to redraw prompt");
    }

    fn show_error(&mut self, message: &str) {
        self.clear_prompt().expect("failed to clear prompt");
        queue!(
            self.stdout,
            MoveToColumn(0),
            Print("[ERROR] "),
            Print(message),
            Print("\r\n"),
        )
        .expect("failed to show error");

        self.prompt_lines = 1;
        self.stdout.flush().expect("failed to flush stdout");
        self.redraw_prompt().expect("failed to redraw prompt");
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.show_message(prompt);
    }

    fn show_status_line(&mut self, message: &str) {
        self.clear_prompt()
            .expect("failed to clear prompt for status");
        queue!(self.stdout, Print(message), Clear(ClearType::UntilNewLine))
            .expect("failed to show status line");
        self.prompt_lines = 1;
        self.stdout.flush().expect("failed to flush stdout");
    }

    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError> {
        if !event::poll(Duration::from_millis(50)).unwrap_or(false) {
            return Ok(None);
        }

        match event::read() {
            Ok(event) => self.handle_event(event, limit),
            Err(_) => Err(UiInputError::Disconnected),
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
            // Only disable raw mode if this instance was the one to enable it.
            // This prevents tests from disabling raw mode for the test runner.
            execute!(self.stdout, Print("\r\n")).ok();
            terminal::disable_raw_mode().expect("failed to disable raw mode");
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use shared::chat::MAX_CHAT_MESSAGE_BYTES;

    // Create a new `TerminalUi` for testing with a fake width
    // and a `Vec<u8>` as the `stdout` buffer.
    fn setup_test_ui(cols: u16) -> TerminalUi<Vec<u8>> {
        TerminalUi {
            stdout: Vec::new(), // Use a simple vector as the writer.
            buffer: String::new(),
            prompt_lines: 1,
            cols,                     // Set the terminal width.
            is_raw_mode_owner: false, // Don't touch global raw mode.
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
        let output = String::from_utf8(ui.stdout.clone()).unwrap();
        assert_eq!(output, "a");
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

        // Backspace on empty buffer should do nothing.
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

        // Should return the line.
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hi");

        assert_eq!(ui.buffer, "");
        // Prompt should be reset to 1 line.
        assert_eq!(ui.prompt_lines, 1);
    }

    #[test]
    fn test_multiline_backspace_unwraps_correctly() {
        // Use a tiny terminal width: 10 columns.
        // Prompt is "> ", which takes 2 columns.
        // This leaves 8 columns for the buffer on the first line.
        let mut ui = setup_test_ui(10);

        // 1. Fill the first line exactly.
        // Buffer: "12345678" (8 chars). Prompt + buffer = 10 chars.
        for c in "12345678".chars() {
            ui.handle_event(key_event(KeyCode::Char(c)), MAX_CHAT_MESSAGE_BYTES)
                .expect("failed to handle char");
        }
        assert_eq!(ui.buffer, "12345678");
        assert_eq!(ui.prompt_lines, 1); // Exactly 1 line.

        // 2. Add one more char to wrap to the second line.
        // Buffer: "123456789" (9 chars).
        ui.handle_event(key_event(KeyCode::Char('9')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle char");
        assert_eq!(ui.buffer, "123456789");
        // Line 1: "> 12345678".
        // Line 2: "9".
        assert_eq!(ui.prompt_lines, 2);

        // 3. Add another char to be safe.
        // Buffer: "1234567890" (10 chars).
        ui.handle_event(key_event(KeyCode::Char('0')), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle char");
        assert_eq!(ui.buffer, "1234567890");
        // Line 1: "> 12345678".
        // Line 2: "90".
        assert_eq!(ui.prompt_lines, 2);

        // 4. Backspace from line 2.
        // Buffer: "123456789".
        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");
        assert_eq!(ui.buffer, "123456789");
        assert_eq!(ui.prompt_lines, 2); // Still 2 lines.

        // 5. THE CRITICAL TEST: Backspace again to "un-wrap".
        // Buffer: "12345678".
        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");

        // Check that the buffer is correct.
        assert_eq!(ui.buffer, "12345678");

        // Check that the UI *knows* it's back to 1 line.
        // This is what was failing previously.
        assert_eq!(ui.prompt_lines, 1);

        // 6. Backspace again, should stay on 1 line.
        // Buffer: "1234567".
        ui.handle_event(key_event(KeyCode::Backspace), MAX_CHAT_MESSAGE_BYTES)
            .expect("failed to handle backspace");
        assert_eq!(ui.buffer, "1234567");
        assert_eq!(ui.prompt_lines, 1);
    }
}
