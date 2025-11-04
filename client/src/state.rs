use shared::auth::Passcode;

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
    InGame,
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
        "Welcome! You are connected." => AuthMessageOutcome::Authenticated,
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
    message_count: u64,
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            state: ClientState::Startup {
                prompt_printed: false,
            },
            first_passcode: None,
            message_count: 0,
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

    pub fn tick_message_counter(&mut self) -> u64 {
        let current = self.message_count;
        self.message_count = self.message_count.saturating_add(1);
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprets_welcome_message() {
        let mut guesses_left = 3;
        let outcome = interpret_auth_message("Welcome! You are connected.", &mut guesses_left);
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
        let outcome = interpret_auth_message("Incorrect passcode. Disconnecting.", &mut guesses_left);
        assert_eq!(guesses_left, 1);
        assert_eq!(
            outcome,
            AuthMessageOutcome::Disconnect("Incorrect passcode (final attempt failed).".to_string())
        );
    }

    #[test]
    fn ignores_unexpected_message() {
        let mut guesses_left = 2;
        let outcome = interpret_auth_message("Some other message", &mut guesses_left);
        assert_eq!(outcome, AuthMessageOutcome::None);
        assert_eq!(guesses_left, 2);
    }
}
