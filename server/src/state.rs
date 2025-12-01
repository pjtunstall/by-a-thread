use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use crate::net::ServerNetworkHandle;
use bincode::{config::standard, serde::encode_to_vec};
use shared::{maze, net::AppChannel, player::Player, protocol::ServerMessage};

pub enum ServerState {
    Lobby(Lobby),
    ChoosingDifficulty(ChoosingDifficulty),
    Countdown(Countdown),
    Game(Game),
}

impl ServerState {
    pub fn name(&self) -> &'static str {
        match self {
            ServerState::Lobby(_) => "Lobby",
            ServerState::ChoosingDifficulty(_) => "ChoosingDifficulty",
            ServerState::Countdown(_) => "Countdown",
            ServerState::Game(_) => "Game",
        }
    }

    pub fn register_connection(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        match self {
            ServerState::Lobby(lobby) => lobby.register_connection(client_id),
            _ => {
                eprintln!(
                    "Client {} connected, but server is not in Lobby state. Informing and closing locally.",
                    client_id
                );

                let message = ServerMessage::ServerInfo {
                    message: "game already started: disconnecting".to_string(),
                };
                let payload = encode_to_vec(&message, standard())
                    .expect("failed to serialize ServerInfo message");

                network.send_message(client_id, AppChannel::ReliableOrdered, payload);
            }
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        match self {
            ServerState::Lobby(lobby) => lobby.remove_client(client_id, network),
            ServerState::ChoosingDifficulty(state) => state.lobby.remove_client(client_id, network),
            ServerState::Countdown(countdown) => countdown.remove_client(client_id, network),
            ServerState::Game(game) => game.remove_client(client_id, network),
        }
    }
}

#[derive(Clone)]
pub struct ChoosingDifficulty {
    pub lobby: Lobby,
    pub difficulty: u8,
    pub host_id: Option<u64>,
}

impl ChoosingDifficulty {
    pub fn new(lobby: &Lobby) -> Self {
        let host_id = lobby
            .host_client_id
            .or_else(|| lobby.usernames.keys().copied().next());
        Self {
            lobby: lobby.clone(),
            difficulty: 1,
            host_id,
        }
    }
    pub fn host_id(&self) -> Option<u64> {
        self.host_id
    }
    pub fn username(&self, client_id: u64) -> Option<&str> {
        self.lobby.username(client_id)
    }
    pub fn set_difficulty(&mut self, level: u8) {
        self.difficulty = level;
    }
}

#[derive(Clone)]
pub struct Countdown {
    pub usernames: HashMap<u64, String>,
    pub players: Vec<Player>,
    pub host_id: Option<u64>,
    pub end_time: Instant,
    pub maze: maze::Maze,
}

impl Countdown {
    pub fn new(
        state: &ChoosingDifficulty,
        players: Vec<Player>,
        end_time: Instant,
        maze: maze::Maze,
    ) -> Self {
        Self {
            usernames: state.lobby.usernames.clone(),
            players,
            host_id: state.host_id,
            end_time,
            maze,
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        if let Some(username) = self.usernames.remove(&client_id) {
            println!(
                "Client {} ({}) disconnected during countdown.",
                client_id, username
            );
            let message = ServerMessage::UserLeft { username };
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize UserLeft");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);
        } else {
            println!(
                "Client {} disconnected during countdown (no username).",
                client_id
            );
        }
        self.players.retain(|p| p.client_id != client_id);

        let host_was_removed = self.host_id == Some(client_id);
        let no_host = self.host_id.is_none();
        if self.players.is_empty() {
            self.host_id = None;
        } else if host_was_removed || no_host {
            if let Some(new_host) = self.players.first() {
                self.host_id = Some(new_host.client_id);
                notify_new_host(network, new_host.client_id);
                println!("Host reassigned to client {}", new_host.client_id);
            }
        }
    }
}

pub struct Game {
    pub players: Vec<Player>,
    pub maze: maze::Maze,
    pub host_id: Option<u64>,
}

impl Game {
    pub fn new(players: Vec<Player>, maze: maze::Maze, host_id: Option<u64>) -> Self {
        Self {
            players,
            maze,
            host_id,
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        if let Some(player) = self
            .players
            .iter()
            .find(|p| p.client_id == client_id)
            .cloned()
        {
            let username = player.name;
            println!(
                "Client {} ({}) disconnected during game.",
                client_id, username
            );
            let message = ServerMessage::UserLeft { username };
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize UserLeft");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);
        }
        self.players.retain(|p| p.client_id != client_id);

        let host_was_removed = self.host_id == Some(client_id);
        let no_host = self.host_id.is_none();
        if self.players.is_empty() {
            self.host_id = None;
        } else if host_was_removed || no_host {
            if let Some(new_host) = self.players.first() {
                self.host_id = Some(new_host.client_id);
                notify_new_host(network, new_host.client_id);
                println!("Host reassigned to client {}", new_host.client_id);
            }
        }
    }
}

#[derive(Clone)]
pub struct Lobby {
    auth_attempts: HashMap<u64, u8>,
    pending_usernames: HashSet<u64>,
    pub usernames: HashMap<u64, String>,
    host_client_id: Option<u64>,
}

fn notify_new_host(network: &mut dyn ServerNetworkHandle, id: u64) {
    let message = ServerMessage::AppointHost;
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize AppointHost");
    network.send_message(id, AppChannel::ReliableOrdered, payload);
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

    pub fn set_host(&mut self, id: u64, network: &mut dyn ServerNetworkHandle) {
        self.host_client_id = Some(id);
        notify_new_host(network, id);
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
            let message = ServerMessage::UserLeft { username };
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize UserLeft");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);
        }

        if self.host_client_id == Some(client_id) {
            if let Some(new_host_id) = self.usernames.keys().cloned().next() {
                self.set_host(new_host_id, network);
                println!("Host disconnected, new host is client {}", new_host_id);
            } else {
                self.host_client_id = None;
                println!("Host left and no clients remain; host cleared.");
            }
        } else if self.host_client_id.is_none() && self.usernames.len() == 1 {
            // All but one user removed and host was unset; promote the remaining user.
            if let Some((&remaining_id, _)) = self.usernames.iter().next() {
                self.set_host(remaining_id, network);
                println!("Host assigned to remaining client {}", remaining_id);
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

    pub fn pending_clients(&self) -> Vec<u64> {
        let mut pending: HashSet<u64> = self.auth_attempts.keys().cloned().collect();
        pending.extend(self.pending_usernames.iter().cloned());
        pending.into_iter().collect()
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
    use bincode::{config::standard, serde::decode_from_slice};
    use glam::Vec3;

    use super::*;
    use crate::test_helpers::MockServerNetwork;
    use shared::protocol::ServerMessage;

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
        assert_eq!(attempts, &mut 0);
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

        let messages = network.get_sent_messages_data(123);
        assert_eq!(messages.len(), 1);

        let msg = decode_from_slice::<ServerMessage, _>(&messages[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::AppointHost = msg {
            // Expected; client will show its own message on receipt.
        } else {
            panic!("expected AppointHost message, got {:?}", msg);
        }
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

    #[test]
    fn countdown_reassigns_host_and_notifies_when_host_leaves() {
        let mut network = MockServerNetwork::new();
        network.add_client(1);
        network.add_client(2);

        let mut countdown = Countdown {
            usernames: HashMap::from([(1, "Alice".to_string()), (2, "Bob".to_string())]),
            players: vec![
                Player::new(
                    0,
                    1,
                    "Alice".to_string(),
                    Vec3::ZERO,
                    shared::player::Color::RED,
                ),
                Player::new(
                    1,
                    2,
                    "Bob".to_string(),
                    Vec3::ZERO,
                    shared::player::Color::BLUE,
                ),
            ],
            host_id: Some(1),
            end_time: Instant::now(),
            maze: maze::Maze::new(maze::Algorithm::Backtrack),
        };

        countdown.remove_client(1, &mut network);

        assert_eq!(countdown.host_id, Some(2));
        let messages = network.get_sent_messages_data(2);
        assert!(
            messages.iter().any(|m| matches!(
                decode_from_slice::<ServerMessage, _>(m, standard())
                    .unwrap()
                    .0,
                ServerMessage::AppointHost
            )),
            "expected AppointHost message to new host"
        );
    }

    #[test]
    fn game_reassigns_host_and_notifies_when_host_leaves() {
        let mut network = MockServerNetwork::new();
        network.add_client(10);
        network.add_client(20);

        let mut game = Game {
            players: vec![
                Player::new(
                    0,
                    10,
                    "Alice".to_string(),
                    Vec3::ZERO,
                    shared::player::Color::RED,
                ),
                Player::new(
                    1,
                    20,
                    "Bob".to_string(),
                    Vec3::ZERO,
                    shared::player::Color::BLUE,
                ),
            ],
            maze: maze::Maze::new(maze::Algorithm::Backtrack),
            host_id: Some(10),
        };

        game.remove_client(10, &mut network);

        assert_eq!(game.host_id, Some(20));
        let messages = network.get_sent_messages_data(20);
        assert!(
            messages.iter().any(|m| matches!(
                decode_from_slice::<ServerMessage, _>(m, standard())
                    .unwrap()
                    .0,
                ServerMessage::AppointHost
            )),
            "expected AppointHost message to new host"
        );
    }
}
