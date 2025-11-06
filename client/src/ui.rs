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
    prompt_lines: u16,
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
            prompt_lines: 1,
        })
    }

    fn redraw_prompt(&mut self) -> io::Result<()> {
        let (cols, _) = terminal::size().unwrap_or((80, 24));
        let prompt = "> ";

        // 1. Move cursor to the start of the *previous* prompt block.
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )?;
        }

        // 2. Clear all *previous* lines.
        for i in 0..self.prompt_lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            if i + 1 < self.prompt_lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1))?;
            }
        }

        // 3. Move back to the start.
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )?;
        }
        queue!(self.stdout, MoveToColumn(0))?;

        // 4. Calculate *new* line count.
        let full = format!("{}{}", prompt, &self.buffer);
        let new_lines = (full.len().max(1) + cols as usize - 1) / cols as usize;
        self.prompt_lines = new_lines as u16;

        // 5. Print new prompt.
        queue!(self.stdout, Print(prompt), Print(&self.buffer))?;

        self.stdout.flush()
    }
}

impl ClientUi for TerminalUi {
    fn show_message(&mut self, message: &str) {
        // 1. Clear the current prompt first. (First before what?)
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .unwrap();
        }
        for i in 0..self.prompt_lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine)).unwrap();
            if i + 1 < self.prompt_lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1)).unwrap();
            }
        }
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .unwrap();
        }

        // 2. Print the message. (Well, duh!)
        queue!(self.stdout, MoveToColumn(0), Print(message), Print("\r\n"),)
            .expect("failed to show message");

        // 3. Redraw the prompt (which is still in the buffer) on the new line
        self.prompt_lines = 1;
        self.redraw_prompt().expect("failed to redraw prompt");
    }

    fn show_error(&mut self, message: &str) {
        // 1. Clear the current prompt first.
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .unwrap();
        }
        for i in 0..self.prompt_lines {
            queue!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine)).unwrap();
            if i + 1 < self.prompt_lines {
                queue!(self.stdout, crossterm::cursor::MoveDown(1)).unwrap();
            }
        }
        if self.prompt_lines > 1 {
            queue!(
                self.stdout,
                crossterm::cursor::MoveUp(self.prompt_lines - 1)
            )
            .unwrap();
        }

        // 2. Print the error.
        queue!(
            self.stdout,
            MoveToColumn(0),
            Print("[ERROR] "),
            Print(message),
            Print("\r\n"),
        )
        .expect("failed to show error");

        // 3. Redraw the prompt (which is still in the buffer) on the new line.
        self.prompt_lines = 1;
        self.stdout.flush().expect("failed to flush stdout");
        self.redraw_prompt().expect("failed to redraw prompt");
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
                        self.prompt_lines = 1;
                        self.redraw_prompt().unwrap();
                        Ok(Some(line))
                    }
                    KeyCode::Char(c) => {
                        self.buffer.push(c);
                        self.redraw_prompt().unwrap();
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
