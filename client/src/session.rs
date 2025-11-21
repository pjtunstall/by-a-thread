use std::collections::HashMap;

use crate::state::ClientState;
use shared::{
    auth::Passcode,
    maze::Maze,
    player::{MAX_USERNAME_LENGTH, Player, UsernameError},
};

pub struct ClientSession {
    pub client_id: u64,
    pub state: ClientState,
    pub first_passcode: Option<Passcode>,
    pub awaiting_initial_roster: bool,
    pub estimated_server_time: f64,
    pub countdown_end_time: Option<f64>,
    pub maze: Option<Maze>,
    pub players: Option<HashMap<u64, Player>>,
    pub input_queue: Vec<String>,
}

impl ClientSession {
    pub fn new(client_id: u64) -> Self {
        Self {
            client_id,
            state: ClientState::Startup {
                prompt_printed: false,
            },
            first_passcode: None,
            awaiting_initial_roster: false,
            estimated_server_time: 0.0,
            countdown_end_time: None,
            maze: None,
            players: None,
            input_queue: Vec::new(),
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

    pub fn has_first_passcode(&self) -> bool {
        self.first_passcode.is_some()
    }

    pub fn expect_initial_roster(&mut self) {
        self.awaiting_initial_roster = true;
    }

    pub fn awaiting_initial_roster(&self) -> bool {
        self.awaiting_initial_roster
    }

    pub fn mark_initial_roster_received(&mut self) {
        self.awaiting_initial_roster = false;
    }

    pub fn with_choosing_username<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut bool) -> R,
    {
        match &mut self.state {
            ClientState::ChoosingUsername { prompt_printed } => Some(f(prompt_printed)),
            _ => None,
        }
    }

    pub fn add_input(&mut self, input: String) {
        self.input_queue.push(input);
    }

    pub fn take_input(&mut self) -> Option<String> {
        if self.input_queue.is_empty() {
            None
        } else {
            Some(self.input_queue.remove(0))
        }
    }

    pub fn is_countdown_active(&self) -> bool {
        matches!(self.state(), ClientState::Countdown) && self.countdown_end_time.is_some()
    }
}

pub fn username_prompt() -> String {
    format!(
        "Choose a username (1-{} characters, letters/numbers/_/- only): ",
        MAX_USERNAME_LENGTH
    )
}

pub fn validate_username_input(input: &str) -> Result<String, UsernameError> {
    shared::player::sanitize_username(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{auth::Passcode, player::UsernameError};

    #[test]
    fn new_session_starts_in_startup_state() {
        let session = ClientSession::new(0);
        assert!(matches!(
            session.state(),
            ClientState::Startup {
                prompt_printed: false
            }
        ));
    }

    #[test]
    fn first_passcode_is_stored_and_cleared() {
        let mut session = ClientSession::new(0);
        let passcode = Passcode {
            bytes: vec![1, 2, 3, 4, 5, 6],
            string: "123456".to_string(),
        };

        session.store_first_passcode(passcode);
        let retrieved = session
            .take_first_passcode()
            .expect("expected stored passcode to exist");
        assert_eq!(retrieved.string, "123456");
        assert!(session.take_first_passcode().is_none());
    }

    #[test]
    fn transition_updates_state() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Connecting);
        assert!(matches!(session.state(), ClientState::Connecting));
        session.transition(ClientState::Disconnected {
            message: "done".to_string(),
        });

        match session.state() {
            ClientState::Disconnected { message } => assert_eq!(message, "done"),
            _ => panic!("unexpected state after transition"),
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
        let validated = validate_username_input("  Player-1  ").expect("valid username expected");
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

    #[test]
    fn input_queue_stores_and_retrieves_in_order() {
        let mut session = ClientSession::new(0);

        session.add_input("message one".to_string());
        session.add_input("message two".to_string());

        assert_eq!(session.take_input(), Some("message one".to_string()));
        assert_eq!(session.take_input(), Some("message two".to_string()));
        assert_eq!(session.take_input(), None);
    }
}
