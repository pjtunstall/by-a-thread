use std::fmt;
use std::io::{Write, stdin, stdout};
use std::sync::mpsc::{self, Receiver};
use std::thread;

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
}

impl TerminalUi {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>();

        thread::spawn(move || {
            loop {
                let mut buffer = String::new();
                match stdin().read_line(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = buffer.trim().to_string();
                        if tx.send(trimmed).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Input thread error: {}", e);
                        break;
                    }
                }
            }
        });

        Self { rx }
    }
}

impl ClientUi for TerminalUi {
    fn show_message(&mut self, message: &str) {
        println!("{}", message);
    }

    fn show_error(&mut self, message: &str) {
        eprintln!("{}", message);
    }

    fn show_prompt(&mut self, prompt: &str) {
        print!("{}", prompt);
        stdout().flush().expect("Failed to flush stdout");
    }

    fn poll_input(&mut self) -> Result<Option<String>, UiInputError> {
        match self.rx.try_recv() {
            Ok(input) => Ok(Some(input)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(UiInputError::Disconnected),
        }
    }
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
