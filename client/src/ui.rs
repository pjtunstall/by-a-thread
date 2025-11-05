use std::fmt;
use std::io::{Stdout, Write, stderr, stdin, stdout};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::{clear, cursor};

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
    rx: Receiver<String>,
    state: Arc<Mutex<PromptState>>,
    stdout: Arc<Mutex<RawTerminal<Stdout>>>,
}

struct PromptState {
    prompt: String,
    prompt_line_count: usize,
    buffer: String,
    displayed: bool,
}

impl TerminalUi {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>();
        let state = Arc::new(Mutex::new(PromptState {
            prompt: String::new(),
            prompt_line_count: 1,
            buffer: String::new(),
            displayed: false,
        }));

        let stdout = stdout()
            .into_raw_mode()
            .expect("Failed to set stdout to raw mode");
        let stdout = Arc::new(Mutex::new(stdout));

        let input_state = Arc::clone(&state);
        let input_stdout = Arc::clone(&stdout);

        thread::spawn(move || {
            let stdin = stdin();
            for key in stdin.lock().keys() {
                match key {
                    Ok(Key::Char('\n')) | Ok(Key::Char('\r')) => {
                        let input = {
                            let mut state = input_state.lock().unwrap();
                            if !state.displayed {
                                continue;
                            }

                            let submission = state.buffer.trim().to_string();
                            state.buffer.clear();

                            if let Ok(mut stdout) = input_stdout.lock() {
                                let _ = Self::clear_prompt_area_internal(
                                    &mut stdout,
                                    state.prompt_line_count,
                                    state.displayed,
                                );
                                let _ = Self::render_prompt(&mut stdout, &state);
                            }

                            submission
                        };

                        if tx.send(input).is_err() {
                            break;
                        }
                    }
                    Ok(Key::Backspace) => {
                        let mut state = input_state.lock().unwrap();
                        if !state.displayed {
                            continue;
                        }

                        state.buffer.pop();

                        if let Ok(mut stdout) = input_stdout.lock() {
                            let _ = Self::clear_prompt_area_internal(
                                &mut stdout,
                                state.prompt_line_count,
                                state.displayed,
                            );
                            let _ = Self::render_prompt(&mut stdout, &state);
                        }
                    }
                    Ok(Key::Ctrl('c')) | Ok(Key::Ctrl('d')) => break,
                    Ok(Key::Char(ch)) => {
                        let mut state = input_state.lock().unwrap();
                        if !state.displayed {
                            continue;
                        }

                        if ch.is_control() {
                            continue;
                        }

                        state.buffer.push(ch);

                        if let Ok(mut stdout) = input_stdout.lock() {
                            let _ = Self::clear_prompt_area_internal(
                                &mut stdout,
                                state.prompt_line_count,
                                state.displayed,
                            );
                            let _ = Self::render_prompt(&mut stdout, &state);
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        Self { rx, state, stdout }
    }
}

impl ClientUi for TerminalUi {
    fn show_message(&mut self, message: &str) {
        let mut state = self.state.lock().unwrap();
        let mut stdout = self.stdout.lock().unwrap();

        let _ =
            Self::clear_prompt_area_internal(&mut stdout, state.prompt_line_count, state.displayed);

        writeln!(stdout, "{}", message).expect("Failed to write message to stdout");

        if state.displayed {
            let _ = Self::render_prompt(&mut stdout, &state);
        } else {
            stdout.flush().expect("Failed to flush stdout");
        }
    }

    fn show_error(&mut self, message: &str) {
        let mut state = self.state.lock().unwrap();
        let mut stdout = self.stdout.lock().unwrap();

        let _ =
            Self::clear_prompt_area_internal(&mut stdout, state.prompt_line_count, state.displayed);

        let mut err = stderr();
        writeln!(err, "{}", message).expect("Failed to write error to stderr");
        err.flush().expect("Failed to flush stderr");

        if state.displayed {
            let _ = Self::render_prompt(&mut stdout, &state);
        } else {
            stdout.flush().expect("Failed to flush stdout");
        }
    }

    fn show_prompt(&mut self, prompt: &str) {
        let mut state = self.state.lock().unwrap();
        let previous_lines = state.prompt_line_count;
        let was_displayed = state.displayed;

        state.prompt = prompt.to_string();
        state.prompt_line_count = count_prompt_lines(prompt);
        state.buffer.clear();
        state.displayed = true;

        let mut stdout = self.stdout.lock().unwrap();

        let _ = Self::clear_prompt_area_internal(&mut stdout, previous_lines, was_displayed);
        let _ = Self::render_prompt(&mut stdout, &state);
    }

    fn poll_input(&mut self) -> Result<Option<String>, UiInputError> {
        match self.rx.try_recv() {
            Ok(input) => Ok(Some(input)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(UiInputError::Disconnected),
        }
    }
}

impl TerminalUi {
    fn clear_prompt_area_internal(
        stdout: &mut RawTerminal<Stdout>,
        line_count: usize,
        displayed: bool,
    ) -> std::io::Result<()> {
        if displayed {
            write!(stdout, "\r")?;
            if line_count > 0 {
                let lines_to_move = line_count.min(u16::MAX as usize) as u16;
                if lines_to_move > 0 {
                    write!(stdout, "{}", cursor::Up(lines_to_move))?;
                }
            }
            write!(stdout, "{}", clear::AfterCursor)?;
        }

        Ok(())
    }

    fn render_prompt(stdout: &mut RawTerminal<Stdout>, state: &PromptState) -> std::io::Result<()> {
        if !state.prompt.is_empty() {
            writeln!(stdout, "{}", state.prompt)?;
        } else {
            writeln!(stdout)?;
        }

        write!(stdout, "> {}", state.buffer)?;
        stdout.flush()
    }
}

fn count_prompt_lines(prompt: &str) -> usize {
    let lines = prompt.split('\n').count();
    if lines == 0 { 1 } else { lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_input_error_has_display_message() {
        assert_eq!(
            UiInputError::Disconnected.to_string(),
            "Input source disconnected"
        );
    }
}
