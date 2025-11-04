use std::collections::HashMap;

pub const MAX_AUTH_ATTEMPTS: u8 = 3;

#[derive(Debug, PartialEq)]
pub enum AuthAttemptOutcome {
    Authenticated,
    TryAgain,
    Disconnect,
}

pub fn evaluate_passcode_attempt(
    passcode: &[u8],
    attempts: &mut u8,
    guess: &[u8],
    max_attempts: u8,
) -> AuthAttemptOutcome {
    if guess == passcode {
        AuthAttemptOutcome::Authenticated
    } else {
        *attempts = attempts.saturating_add(1);
        if *attempts >= max_attempts {
            AuthAttemptOutcome::Disconnect
        } else {
            AuthAttemptOutcome::TryAgain
        }
    }
}

pub struct ServerState {
    auth_attempts: HashMap<u64, u8>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            auth_attempts: HashMap::new(),
        }
    }

    pub fn register_connection(&mut self, client_id: u64) {
        self.auth_attempts.insert(client_id, 0);
    }

    pub fn remove_client(&mut self, client_id: u64) {
        self.auth_attempts.remove(&client_id);
    }

    pub fn authentication_attempts(&mut self, client_id: u64) -> Option<&mut u8> {
        self.auth_attempts.get_mut(&client_id)
    }

    pub fn is_authenticating(&self, client_id: u64) -> bool {
        self.auth_attempts.contains_key(&client_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_authentication_does_not_increment_attempts() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 0;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &passcode, 3);
        assert_eq!(outcome, AuthAttemptOutcome::Authenticated);
        assert_eq!(attempts, 0);
    }

    #[test]
    fn incorrect_attempt_requests_retry() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 0;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], 3);
        assert_eq!(outcome, AuthAttemptOutcome::TryAgain);
        assert_eq!(attempts, 1);
    }

    #[test]
    fn max_attempts_triggers_disconnect() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 2;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], 3);
        assert_eq!(outcome, AuthAttemptOutcome::Disconnect);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn attempts_do_not_overflow_past_u8_max() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = u8::MAX - 1;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], u8::MAX);
        assert_eq!(attempts, u8::MAX);
        assert_eq!(outcome, AuthAttemptOutcome::Disconnect);
    }

    #[test]
    fn register_connection_initializes_attempts() {
        let mut state = ServerState::new();
        state.register_connection(42);

        let attempts = state
            .authentication_attempts(42)
            .expect("expected attempts entry");
        assert_eq!(*attempts, 0);
        assert!(state.is_authenticating(42));
    }

    #[test]
    fn remove_client_clears_authentication_state() {
        let mut state = ServerState::new();
        state.register_connection(99);
        state.remove_client(99);

        assert!(!state.is_authenticating(99));
        assert!(state.authentication_attempts(99).is_none());
    }

    #[test]
    fn unknown_client_has_no_authentication_entry() {
        let mut state = ServerState::new();
        assert!(state.authentication_attempts(123).is_none());
        assert!(!state.is_authenticating(123));
    }

    #[test]
    fn re_registering_client_resets_attempt_counter() {
        let mut state = ServerState::new();
        state.register_connection(7);
        let attempts = state
            .authentication_attempts(7)
            .expect("expected attempts entry");
        *attempts = 2;

        state.register_connection(7);

        let attempts = state
            .authentication_attempts(7)
            .expect("expected reset attempts entry");
        assert_eq!(*attempts, 0);
    }

    #[test]
    fn custom_max_attempt_threshold_disconnects_immediately() {
        let passcode = [9, 9, 9, 9, 9, 9];
        let mut attempts = 0;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], 1);

        assert_eq!(outcome, AuthAttemptOutcome::Disconnect);
        assert_eq!(attempts, 1);
    }
}
