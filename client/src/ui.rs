use std::fmt;
use std::io::{self, Stdout, Write, stdout};
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
    fn poll_input(&mut self) -> Result<Option<String>, UiInputError>;
}

pub struct TerminalUi {
    stdout: Stdout,
    buffer: String,
}

impl TerminalUi {
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = stdout();
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
        })
    }

    fn redraw_prompt(&mut self) -> io::Result<()> {
        // Calculate how many lines the prompt + buffer occupy
        let (cols, _) = terminal::size().unwrap_or((80, 24));
        let prompt = "> ";
        let full = format!("{}{}", prompt, &self.buffer);
        let lines = (full.len() as u16 + cols - 1) / cols;

        // Move cursor up to the first line of the prompt
        if lines > 1 {
            queue!(self.stdout, crossterm::cursor::MoveUp(lines - 1))?;
        }
        // Clear all lines the prompt occupies
        for i in 0..lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            if i + 1 < lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1))?;
            }
        }
        // After clearing, move cursor up to the first line
        if lines > 1 {
            queue!(self.stdout, crossterm::cursor::MoveUp(lines - 1))?;
        }
        // Print prompt and buffer
        queue!(
            self.stdout,
            MoveToColumn(0),
            Print(prompt),
            Print(&self.buffer)
        )?;
        self.stdout.flush()
    }
}

impl ClientUi for TerminalUi {
    fn show_message(&mut self, message: &str) {
        queue!(
            self.stdout,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(message),
            Print("\r\n"),
            MoveToColumn(0),
            Print("> "),
            Print(&self.buffer)
        )
        .expect("failed to show message");
        self.stdout.flush().expect("failed to flush stdout");
    }

    fn show_error(&mut self, message: &str) {
        queue!(
            self.stdout,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print("[ERROR] "),
            Print(message),
            Print("\r\n"),
            MoveToColumn(0),
            Print("> "),
            Print(&self.buffer)
        )
        .expect("failed to show error");
        self.stdout.flush().expect("failed to flush stdout");
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.show_message(prompt);
    }

    fn poll_input(&mut self) -> Result<Option<String>, UiInputError> {
        if !event::poll(Duration::from_millis(50)).unwrap_or(false) {
            return Ok(None);
        }

        match event::read() {
            Ok(Event::Key(key_event)) => {
                if key_event.modifiers == KeyModifiers::CONTROL
                    && key_event.code == KeyCode::Char('c')
                {
                    return Err(UiInputError::Disconnected);
                }

                match key_event.code {
                    KeyCode::Enter => {
                        let line = self.buffer.drain(..).collect();
                        queue!(self.stdout, Print("\r\n")).unwrap();
                        self.redraw_prompt().unwrap();
                        Ok(Some(line))
                    }
                    KeyCode::Char(c) => {
                        self.buffer.push(c);
                        queue!(self.stdout, Print(c)).unwrap();
                        self.stdout.flush().unwrap();
                        Ok(None)
                    }
                    KeyCode::Backspace => {
                        if self.buffer.pop().is_some() {
                            self.redraw_prompt().unwrap();
                        }
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            Ok(Event::Resize(_, _)) => {
                self.redraw_prompt().unwrap();
                Ok(None)
            }
            Err(_) => Err(UiInputError::Disconnected),
            _ => Ok(None),
        }
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        execute!(self.stdout, Print("\r\n")).ok();
        terminal::disable_raw_mode().expect("Failed to disable raw mode");
    }
}
