use shared::auth::Passcode;
use shared::chat::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username};
use std::io::Write;

pub const MAX_ATTEMPTS: u8 = 3;

pub enum ClientState {
    Startup {
        prompt_printed: bool,
    },
    Connecting,
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
    },
    ChoosingUsername {
        prompt_printed: bool,
        awaiting_confirmation: bool,
    },
    InChat,
    Disconnected {
        message: String,
    },
}

#[derive(Debug, PartialEq)]
pub enum AuthMessageOutcome {
    Authenticated,
    RequestNewGuess(u8),
    Disconnect(String),
    None,
}

pub fn interpret_auth_message(text: &str, guesses_left: &mut u8) -> AuthMessageOutcome {
    match text {
        "Authentication successful! Please enter a username (1-16 characters)." => {
            AuthMessageOutcome::Authenticated
        }
        "Incorrect passcode. Try again." => {
            *guesses_left = guesses_left.saturating_sub(1);
            AuthMessageOutcome::RequestNewGuess(*guesses_left)
        }
        "Incorrect passcode. Disconnecting." => {
            AuthMessageOutcome::Disconnect("Incorrect passcode (final attempt failed).".to_string())
        }
        _ => AuthMessageOutcome::None,
    }
}

pub struct ClientSession {
    state: ClientState,
    first_passcode: Option<Passcode>,
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            state: ClientState::Startup {
                prompt_printed: false,
            },
            first_passcode: None,
        }
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ClientState {
        &mut self.state
    }

    pub fn transition(&mut self, new_state: ClientState) {
        self.state = new_state;
    }

    pub fn store_first_passcode(&mut self, passcode: Passcode) {
        self.first_passcode = Some(passcode);
    }

    pub fn take_first_passcode(&mut self) -> Option<Passcode> {
        self.first_passcode.take()
    }
}

pub fn prompt_for_username() {
    print!(
        "Choose a username (1-{} characters, letters/numbers/_/- only): ",
        MAX_USERNAME_LENGTH
    );
    std::io::stdout()
        .flush()
        .expect("Failed to flush stdout while prompting for username");
}

pub fn validate_username_input(input: &str) -> Result<String, UsernameError> {
    sanitize_username(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::auth::Passcode;

    #[test]
    fn interprets_welcome_message() {
        let mut guesses_left = 3;
        let outcome = interpret_auth_message(
            "Authentication successful! Please enter a username (1-16 characters).",
            &mut guesses_left,
        );
        assert_eq!(outcome, AuthMessageOutcome::Authenticated);
        assert_eq!(guesses_left, 3);
    }

    #[test]
    fn interprets_try_again_message() {
        let mut guesses_left = 3;
        let outcome = interpret_auth_message("Incorrect passcode. Try again.", &mut guesses_left);
        assert_eq!(outcome, AuthMessageOutcome::RequestNewGuess(2));
        assert_eq!(guesses_left, 2);
    }

    #[test]
    fn interprets_disconnect_message() {
        let mut guesses_left = 1;
        let outcome =
            interpret_auth_message("Incorrect passcode. Disconnecting.", &mut guesses_left);
        assert_eq!(guesses_left, 1);
        assert_eq!(
            outcome,
            AuthMessageOutcome::Disconnect(
                "Incorrect passcode (final attempt failed).".to_string()
            )
        );
    }

    #[test]
    fn ignores_unexpected_message() {
        let mut guesses_left = 2;
        let outcome = interpret_auth_message("Some other message", &mut guesses_left);
        assert_eq!(outcome, AuthMessageOutcome::None);
        assert_eq!(guesses_left, 2);
    }

    #[test]
    fn try_again_message_saturates_guesses_at_zero() {
        let mut guesses_left = 0;
        let outcome = interpret_auth_message("Incorrect passcode. Try again.", &mut guesses_left);
        assert_eq!(outcome, AuthMessageOutcome::RequestNewGuess(0));
        assert_eq!(guesses_left, 0);
    }

    #[test]
    fn new_session_starts_in_startup_state() {
        let session = ClientSession::new();
        assert!(matches!(
            session.state(),
            ClientState::Startup {
                prompt_printed: false
            }
        ));
    }

    #[test]
    fn first_passcode_is_stored_and_cleared() {
        let mut session = ClientSession::new();
        let passcode = Passcode {
            bytes: vec![1, 2, 3, 4, 5, 6],
            string: "123456".to_string(),
        };

        session.store_first_passcode(passcode);
        let retrieved = session
            .take_first_passcode()
            .expect("expected stored passcode");
        assert_eq!(retrieved.string, "123456");
        assert!(session.take_first_passcode().is_none());
    }

    #[test]
    fn transition_updates_state() {
        let mut session = ClientSession::new();
        session.transition(ClientState::Connecting);
        assert!(matches!(session.state(), ClientState::Connecting));
        session.transition(ClientState::Disconnected {
            message: "done".to_string(),
        });

        match session.state() {
            ClientState::Disconnected { message } => assert_eq!(message, "done"),
            _ => panic!("Unexpected state after transition"),
        }
    }

    #[test]
    fn username_validation_rejects_invalid_values() {
        assert_eq!(validate_username_input(""), Err(UsernameError::Empty));
        assert_eq!(validate_username_input("    "), Err(UsernameError::Empty));
        assert_eq!(
            validate_username_input("user!"),
            Err(UsernameError::InvalidCharacter('!'))
        );
    }

    #[test]
    fn username_validation_accepts_trimmed_valid_value() {
        let validated = validate_username_input("  Player-1  ").expect("expected valid username");
        assert_eq!(validated, "Player-1");
    }

    #[test]
    fn username_validation_respects_length_limit() {
        let too_long = "abcdefghijklmnopq";
        assert_eq!(
            validate_username_input(too_long),
            Err(UsernameError::TooLong)
        );
    }
}
