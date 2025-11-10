use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crate::ServerNetworkHandle;
use shared::net::AppChannel;

pub const MAX_AUTH_ATTEMPTS: u8 = 3;

pub struct Countdown {
    usernames: HashMap<u64, String>,
    pub end_time: Instant,
}

impl Countdown {
    pub fn new(lobby: &Lobby, end_time: Instant) -> Self {
        Self {
            usernames: lobby.usernames.clone(),
            end_time,
        }
    }

    pub fn remove_client(&mut self, client_id: u64, _network: &mut dyn ServerNetworkHandle) {
        println!("Client {} disconnected during countdown.", client_id);
        // TODO: Handle player/host disconnects.
        let _ = self.usernames.remove(&client_id);
    }
}

pub struct InGame {
    // TODO: Add fields, e.g., game state, players.
}

impl InGame {
    pub fn new() -> Self {
        Self {}
    }

    pub fn remove_client(&mut self, client_id: u64, _network: &mut dyn ServerNetworkHandle) {
        println!("Client {} disconnected during game.", client_id);
        // TODO: Handle player/host disconnects.
    }
}

pub enum ServerState {
    Lobby(Lobby),
    Countdown(Countdown),
    InGame(InGame),
}

impl ServerState {
    pub fn register_connection(&mut self, client_id: u64) {
        if let ServerState::Lobby(lobby) = self {
            lobby.register_connection(client_id);
        } else {
            println!(
                "Client {} connected, but server is not in Lobby. Ignoring.",
                client_id
            );
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        match self {
            ServerState::Lobby(lobby) => lobby.remove_client(client_id, network),
            ServerState::Countdown(countdown) => countdown.remove_client(client_id, network),
            ServerState::InGame(in_game) => in_game.remove_client(client_id, network),
        }
    }
}

pub struct Lobby {
    auth_attempts: HashMap<u64, u8>, // Connected users who have yet to enter the correct passcode.
    pending_usernames: HashSet<u64>, // Users who are authenticated but have not yet provided a username.
    usernames: HashMap<u64, String>, // Authenticated users with registered usernames.
    host_client_id: Option<u64>,
}

impl Lobby {
    pub fn new() -> Self {
        Self {
            auth_attempts: HashMap::new(),
            pending_usernames: HashSet::new(),
            usernames: HashMap::new(),
            host_client_id: None,
        }
    }

    pub fn transition_to_countdown(&self, duration: Duration) -> Countdown {
        println!(
            "-> Transitioning Lobby to Countdown. Moving {} usernames...",
            self.usernames.len()
        );

        Countdown {
            usernames: self.usernames.clone(),
            end_time: Instant::now()
                .checked_add(duration)
                .expect("failed to get end time for countdown"),
        }
    }

    pub fn set_host(&mut self, id: u64, network: &mut dyn ServerNetworkHandle) {
        self.host_client_id = Some(id);
        network.send_message(
            id,
            AppChannel::ReliableOrdered,
            "  You have been appointed host.\r\n  Press TAB to start a game."
                .as_bytes()
                .to_vec(),
        );
    }

    pub fn is_host(&self, client_id: u64) -> bool {
        match self.host_client_id {
            Some(host_id) => host_id == client_id,
            None => false,
        }
    }

    pub fn register_connection(&mut self, client_id: u64) {
        self.auth_attempts.insert(client_id, 0);
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        self.auth_attempts.remove(&client_id);
        self.pending_usernames.remove(&client_id);

        let name_removed = self.usernames.remove(&client_id);

        if let Some(username) = name_removed {
            let message = format!("{} left the chat.", username);
            network.broadcast_message(AppChannel::ReliableOrdered, message.into_bytes());
        }

        if self.host_client_id == Some(client_id) {
            if let Some(new_host_id) = self.usernames.keys().cloned().next() {
                self.set_host(new_host_id, network);
                println!("Host disconnected, new host is client {}", new_host_id);
            } else {
                self.host_client_id = None;
                println!("Host left and no clients remain; host cleared.");
            }
        }
    }

    pub fn authentication_attempts(&mut self, client_id: u64) -> Option<&mut u8> {
        self.auth_attempts.get_mut(&client_id)
    }

    pub fn is_authenticating(&self, client_id: u64) -> bool {
        self.auth_attempts.contains_key(&client_id)
    }

    pub fn mark_authenticated(&mut self, client_id: u64) {
        self.auth_attempts.remove(&client_id);
        self.pending_usernames.insert(client_id);
    }

    pub fn needs_username(&self, client_id: u64) -> bool {
        self.pending_usernames.contains(&client_id)
    }

    pub fn register_username(&mut self, client_id: u64, username: &str) -> Option<&str> {
        if self.pending_usernames.remove(&client_id) {
            self.usernames.insert(client_id, username.to_string());
        }
        self.usernames.get(&client_id).map(|s| s.as_str())
    }

    pub fn username(&self, client_id: u64) -> Option<&str> {
        self.usernames.get(&client_id).map(|s| s.as_str())
    }

    pub fn is_username_taken(&self, username: &str) -> bool {
        self.usernames
            .values()
            .any(|existing| existing.eq_ignore_ascii_case(username))
    }

    pub fn usernames_except(&self, client_id: u64) -> Vec<String> {
        self.usernames
            .iter()
            .filter_map(|(&id, name)| {
                if id != client_id {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockServerNetwork;

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
        let outcome =
            evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], u8::MAX);
        assert_eq!(attempts, u8::MAX);
        assert_eq!(outcome, AuthAttemptOutcome::Disconnect);
    }

    #[test]
    fn register_connection_initializes_attempts() {
        let mut state = Lobby::new();
        state.register_connection(42);

        let attempts = state
            .authentication_attempts(42)
            .expect("expected attempts entry");
        assert_eq!(*attempts, 0);
        assert!(state.is_authenticating(42));
    }

    #[test]
    fn remove_client_clears_authentication_state() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        state.register_connection(99);

        state.remove_client(99, &mut network);

        assert!(!state.is_authenticating(99));
        assert!(state.authentication_attempts(99).is_none());
        assert!(state.username(99).is_none());
    }

    #[test]
    fn mark_authenticated_moves_client_to_pending_username() {
        let mut state = Lobby::new();
        state.register_connection(1);
        state.mark_authenticated(1);

        assert!(!state.is_authenticating(1));
        assert!(state.needs_username(1));
    }

    #[test]
    fn register_username_adds_user_and_removes_pending() {
        let mut state = Lobby::new();
        state.register_connection(5);
        state.mark_authenticated(5);

        state
            .register_username(5, "PlayerOne")
            .expect("expected username to register");

        assert!(!state.needs_username(5));
        assert_eq!(state.username(5), Some("PlayerOne"));
    }

    #[test]
    fn username_taken_checks_existing_names_case_insensitively() {
        let mut state = Lobby::new();
        state.register_connection(10);
        state.mark_authenticated(10);
        state.register_username(10, "PlayerOne");

        assert!(state.is_username_taken("playerone"));
        assert!(!state.is_username_taken("SomeoneElse"));
    }

    #[test]
    fn usernames_except_excludes_requested_client() {
        let mut state = Lobby::new();
        for (id, name) in [(1, "Alpha"), (2, "Beta"), (3, "Gamma")] {
            state.register_connection(id);
            state.mark_authenticated(id);
            state.register_username(id, name);
        }

        let mut others = state.usernames_except(2);
        others.sort();
        assert_eq!(others, vec!["Alpha".to_string(), "Gamma".to_string()]);
    }

    #[test]
    fn test_set_host_updates_state() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();

        state.set_host(123, &mut network);

        assert_eq!(state.host_client_id, Some(123));
    }

    #[test]
    fn test_set_host_sends_message_to_new_host() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(123);

        state.set_host(123, &mut network);

        let messages = network.get_sent_messages(123);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("You have been appointed host"));
    }

    #[test]
    fn test_remove_last_client_with_username_clears_host() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();

        state.usernames.insert(1, "Alice".to_string());
        state.set_host(1, &mut network);
        assert_eq!(state.host_client_id, Some(1));

        state.remove_client(1, &mut network);

        assert_eq!(state.host_client_id, None);
    }
}
