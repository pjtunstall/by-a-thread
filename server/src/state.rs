use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    net::ServerNetworkHandle,
    player::{ServerPlayer, Status},
};
use common::{
    bullets::Bullet,
    constants::TICK_SECS,
    maze::Maze,
    net::AppChannel,
    player::{WirePlayerLocal, WirePlayerRemote},
    protocol::{
        AfterGameExitReason, AfterGameLeaderboardEntry, GAME_ALREADY_STARTED_MESSAGE,
        ServerMessage,
    },
    snapshot::{InitialData, Snapshot},
};

pub struct Game {
    pub maze: Maze,
    pub players: Vec<ServerPlayer>,
    pub client_id_to_index: HashMap<u64, usize>,
    pub current_tick: u64,
    pub game_start_tick: u64,
    pub bullets: Vec<Bullet>,
    pub next_bullet_id: u32,
    pub after_game_chat_clients: HashSet<u64>,
    pub leaderboard_sent: bool,
    net_stats: NetStats,
}

impl Game {
    pub fn new(initial_data: InitialData) -> Self {
        let current_tick = (common::time::now_as_secs_f64() / TICK_SECS) as u64;
        let maze = initial_data.maze;
        let mut client_id_to_index = HashMap::new();
        let players = initial_data
            .players
            .into_iter()
            .map(|player| {
                client_id_to_index.insert(player.client_id, player.index);
                ServerPlayer::new(player, current_tick)
            })
            .collect();

        Self {
            maze,
            players,
            client_id_to_index,
            current_tick,
            game_start_tick: current_tick,
            bullets: Vec::new(),
            next_bullet_id: 0,
            after_game_chat_clients: HashSet::new(),
            leaderboard_sent: false,
            net_stats: NetStats::new(),
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        if let Some(&index) = self.client_id_to_index.get(&client_id) {
            let player = &mut self.players[index];
            let name = player.name.clone();
            println!(
                "Client {} ({}) disconnected during game.",
                client_id, player.name
            );
            self.players[index].status = Status::Disconnected;
            if self.players[index].exit_tick.is_none() {
                self.players[index].exit_tick = Some(self.current_tick);
            }
            let message = ServerMessage::UserLeft { username: name };
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize UserLeft");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);

            // If there are no connected players left, exit.
            self.client_id_to_index.remove(&client_id);
            self.after_game_chat_clients.remove(&client_id);
            if self.client_id_to_index.is_empty() {
                println!("All players have disconnected. Server exiting...");
                std::process::exit(0);
            }

            self.send_leaderboard_if_ready(network);
        } else {
            panic!("attempted to remove unknown client: {}", client_id);
        }
    }

    pub fn snapshot_for(&self, i: usize) -> Snapshot {
        let local = WirePlayerLocal::from(self.players[i].state);

        let remote = self
            .players
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, p)| WirePlayerRemote::from(p.state))
            .collect();

        Snapshot { local, remote }
    }

    pub fn note_ingress_bytes(&mut self, bytes: usize) {
        self.net_stats.ingress_bytes = self.net_stats.ingress_bytes.saturating_add(bytes as u64);
    }

    pub fn note_egress_bytes(&mut self, bytes: usize) {
        self.net_stats.egress_bytes = self.net_stats.egress_bytes.saturating_add(bytes as u64);
    }

    pub fn log_network_stats_if_ready(&mut self) {
        self.net_stats.log_if_ready();
    }

    pub fn send_leaderboard_if_ready(&mut self, network: &mut dyn ServerNetworkHandle) {
        if self.leaderboard_sent {
            return;
        }

        if self.after_game_chat_clients.len() != self.client_id_to_index.len() {
            return;
        }

        let entries = self.build_leaderboard_entries();
        let message = ServerMessage::AfterGameLeaderboard { entries };
        let payload =
            encode_to_vec(&message, standard()).expect("failed to serialize AfterGameLeaderboard");
        let payload_len = payload.len();
        let mut egress_bytes = 0usize;

        for client_id in &self.after_game_chat_clients {
            egress_bytes = egress_bytes.saturating_add(payload_len);
            network.send_message(*client_id, AppChannel::ReliableOrdered, payload.clone());
        }

        self.note_egress_bytes(egress_bytes);
        self.leaderboard_sent = true;
    }

    fn build_leaderboard_entries(&self) -> Vec<AfterGameLeaderboardEntry> {
        let mut entries = self
            .players
            .iter()
            .map(|player| {
                let end_tick = player.exit_tick.unwrap_or(self.current_tick);
                let ticks_survived = end_tick.saturating_sub(self.game_start_tick);
                let exit_reason = match player.status {
                    Status::Disconnected => AfterGameExitReason::Disconnected,
                    Status::Dead | Status::Alive => AfterGameExitReason::Slain,
                };
                AfterGameLeaderboardEntry {
                    username: player.name.clone(),
                    ticks_survived,
                    exit_reason,
                }
            })
            .collect::<Vec<_>>();

        entries.sort_by(|a, b| b.ticks_survived.cmp(&a.ticks_survived));
        if let Some(winner) = entries.first_mut() {
            winner.exit_reason = AfterGameExitReason::Winner;
        }
        entries
    }
}

struct NetStats {
    ingress_bytes: u64,
    egress_bytes: u64,
    window_start: Instant,
}

impl NetStats {
    const WINDOW: Duration = Duration::from_secs(3);

    fn new() -> Self {
        Self {
            ingress_bytes: 0,
            egress_bytes: 0,
            window_start: Instant::now(),
        }
    }

    fn log_if_ready(&mut self) {
        let elapsed = self.window_start.elapsed();
        if elapsed < Self::WINDOW {
            return;
        }

        let seconds = elapsed.as_secs_f64();
        let ingress_rate = self.ingress_bytes as f64 / seconds;
        let egress_rate = self.egress_bytes as f64 / seconds;

        println!(
            "Network average over {:.1}s: ingress {}, egress {}.",
            seconds,
            format_bytes_per_second(ingress_rate),
            format_bytes_per_second(egress_rate)
        );

        self.ingress_bytes = 0;
        self.egress_bytes = 0;
        self.window_start = Instant::now();
    }
}

fn format_bytes_per_second(bytes_per_second: f64) -> String {
    const KIBIBYTE: f64 = 1024.0;
    const MEBIBYTE: f64 = 1024.0 * 1024.0;

    if bytes_per_second >= MEBIBYTE {
        format!("{:.1} MB/s", bytes_per_second / MEBIBYTE)
    } else if bytes_per_second >= KIBIBYTE {
        format!("{:.1} kB/s", bytes_per_second / KIBIBYTE)
    } else {
        format!("{:.0} B/s", bytes_per_second)
    }
}

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
                    "client {} connected, but server is not in lobby state; informing, then disconnecting them",
                    client_id
                );

                let message = ServerMessage::ServerInfo {
                    message: GAME_ALREADY_STARTED_MESSAGE.to_string(),
                };
                let payload = encode_to_vec(&message, standard())
                    .expect("failed to serialize ServerInfo message");

                network.send_message(client_id, AppChannel::ReliableOrdered, payload);
                network.disconnect(client_id);
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
    pub host_id: Option<u64>,
    pub end_time: Instant,
    pub game_data: InitialData,
}

impl Countdown {
    pub fn new(state: &ChoosingDifficulty, end_time: Instant, game_data: InitialData) -> Self {
        Self {
            usernames: state.lobby.usernames.clone(),
            host_id: state.host_id,
            end_time,
            game_data,
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
        // Mark player as disconnected instead of removing to preserve indices.
        if let Some(player) = self
            .game_data
            .players
            .iter_mut()
            .find(|p| p.client_id == client_id)
        {
            player.disconnected = true;
        }

        let host_was_removed = self.host_id == Some(client_id);
        let no_host = self.host_id.is_none();
        let has_connected_players = self.game_data.players.iter().any(|p| !p.disconnected);

        if !has_connected_players {
            self.host_id = None;
        } else if host_was_removed || no_host {
            if let Some(new_host) = self.game_data.players.iter().find(|p| !p.disconnected) {
                self.host_id = Some(new_host.client_id);
                notify_new_host(network, new_host.client_id);
                println!("Host reassigned to client {}", new_host.client_id);
            }
        }
    }
}

#[derive(Clone)]
pub struct Lobby {
    pub usernames: HashMap<u64, String>,
    auth_attempts: HashMap<u64, u8>,
    pending_usernames: HashSet<u64>,
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

    use super::*;
    use crate::test_helpers::MockServerNetwork;
    use common::protocol::ServerMessage;

    #[test]
    fn register_connection_disconnects_when_not_in_lobby() {
        let mut network = MockServerNetwork::new();
        network.add_client(7);

        let usernames = HashMap::new();
        let game_data = InitialData::new(&usernames, 1);
        let mut state = ServerState::Countdown(Countdown {
            usernames,
            host_id: None,
            end_time: Instant::now(),
            game_data,
        });

        state.register_connection(7, &mut network);

        let messages = network.get_sent_messages_data(7);
        assert_eq!(messages.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&messages[0], standard())
            .expect("failed to deserialize server message")
            .0;
        if let ServerMessage::ServerInfo { message } = msg {
            assert_eq!(message, GAME_ALREADY_STARTED_MESSAGE);
        } else {
            panic!("expected ServerInfo message, got {:?}", msg);
        }
        assert_eq!(network.disconnected_clients, vec![7]);
    }

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
            // This is the expected variant; client will show its own message on receipt.
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

        let usernames = HashMap::from([(1, "Alice".to_string()), (2, "Bob".to_string())]);
        let game_data = InitialData::new(&usernames, 1);

        let mut countdown = Countdown {
            usernames,
            host_id: Some(1),
            end_time: Instant::now(),
            game_data,
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
}
