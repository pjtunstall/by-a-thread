use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use rand::{Rng as _, rng};

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
    player::{COLORS, Color, WirePlayerLocal, WirePlayerRemote},
    protocol::{
        GAME_ALREADY_STARTED_MESSAGE, PlayerRosterEntry, PostGameExitReason,
        PostGameLeaderboardEntry, ServerMessage,
    },
    snapshot::{InitialData, Snapshot},
};

pub enum ServerState {
    Lobby(Lobby),
    ChoosingDifficulty(ChoosingDifficulty),
    Countdown(Countdown),
    Game(Game),
    Ending,
}

impl ServerState {
    pub fn name(&self) -> &'static str {
        match self {
            ServerState::Lobby(_) => "Lobby",
            ServerState::ChoosingDifficulty(_) => "ChoosingDifficulty",
            ServerState::Countdown(_) => "Countdown",
            ServerState::Game(_) => "Game",
            ServerState::Ending => "Ending",
        }
    }

    pub fn register_connection(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        match self {
            ServerState::Lobby(lobby) => lobby.register_connection(client_id, network),
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
            ServerState::ChoosingDifficulty(state) => state.remove_client(client_id, network),
            ServerState::Countdown(countdown) => countdown.remove_client(client_id, network),
            ServerState::Game(game) => game.remove_client(client_id, network),
            ServerState::Ending => {}
        }
    }
}

pub struct Game {
    pub maze: Maze,
    pub players: Vec<ServerPlayer>,
    pub client_id_to_index: HashMap<u64, usize>,
    pub current_tick: u64,
    pub game_start_tick: u64,
    pub bullets: Vec<Bullet>,
    pub next_bullet_id: u32,
    pub post_game_chat_clients: HashSet<u64>,
    pub leaderboard_sent: bool,
    pub net_stats: NetStats,
    pub exit_coords: Option<(usize, usize)>,
    pub timer_duration: f32,
    pub timer_start_time: f64,
    pub timer_expiration_tick: Option<u64>,
    pub is_solo_mode: bool,
    pub winner_index: Option<usize>,
    pub post_game_start_time: Option<Instant>,
}

impl Game {
    pub fn new(initial_data: InitialData) -> Self {
        let current_tick = (common::time::now_as_secs_f64() / TICK_SECS) as u64;
        let maze = initial_data.maze;
        let timer_duration = initial_data.timer_duration;
        let mut client_id_to_index = HashMap::new();
        let players: Vec<ServerPlayer> = initial_data
            .players
            .into_iter()
            .map(|player| {
                client_id_to_index.insert(player.client_id, player.index);
                ServerPlayer::new(player, current_tick)
            })
            .collect();

        let is_solo_mode = players.len() == 1;
        let timer_start_time = common::time::now_as_secs_f64();

        Self {
            maze,
            players,
            client_id_to_index,
            current_tick,
            game_start_tick: current_tick,
            bullets: Vec::new(),
            next_bullet_id: 0,
            post_game_chat_clients: HashSet::new(),
            leaderboard_sent: false,
            net_stats: NetStats::new(),
            exit_coords: initial_data.exit_coords,
            timer_duration,
            timer_start_time,
            timer_expiration_tick: None,
            is_solo_mode,
            winner_index: None,
            post_game_start_time: None,
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
            let recipients_count = self.client_id_to_index.len();
            self.note_egress_bytes(payload.len().saturating_mul(recipients_count));
            network.broadcast_message(AppChannel::ReliableOrdered, payload);

            // If there are no connected players left, exit.
            self.client_id_to_index.remove(&client_id);
            self.post_game_chat_clients.remove(&client_id);
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
            .filter(|&(j, p)| j != i && matches!(p.status, crate::player::Status::Alive))
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

    pub fn send_leaderboard_if_ready(&mut self, network: &mut dyn ServerNetworkHandle) {
        if self.leaderboard_sent {
            return;
        }

        if self.post_game_chat_clients.len() != self.client_id_to_index.len() {
            return;
        }

        let entries = self.build_leaderboard_entries();
        let message = ServerMessage::PostGameLeaderboard { entries };
        let payload =
            encode_to_vec(&message, standard()).expect("failed to serialize PostGameLeaderboard");
        let payload_len = payload.len();
        let mut egress_bytes = 0usize;

        for client_id in &self.post_game_chat_clients {
            egress_bytes = egress_bytes.saturating_add(payload_len);
            network.send_message(*client_id, AppChannel::ReliableOrdered, payload.clone());
        }

        self.note_egress_bytes(egress_bytes);
        self.leaderboard_sent = true;
    }

    pub fn force_send_leaderboard(&mut self, network: &mut dyn ServerNetworkHandle) {
        if self.leaderboard_sent {
            return;
        }
        let entries = self.build_leaderboard_entries();
        let message = ServerMessage::PostGameLeaderboard { entries };
        let payload =
            encode_to_vec(&message, standard()).expect("failed to serialize PostGameLeaderboard");
        let payload_len = payload.len();
        for client_id in self.client_id_to_index.keys() {
            network.send_message(*client_id, AppChannel::ReliableOrdered, payload.clone());
        }
        self.note_egress_bytes(payload_len.saturating_mul(self.client_id_to_index.len()));
        self.leaderboard_sent = true;
    }

    fn build_leaderboard_entries(&self) -> Vec<PostGameLeaderboardEntry> {
        let mut entries = self
            .players
            .iter()
            .map(|player| {
                let end_tick = player.exit_tick.unwrap_or(self.current_tick);
                let ticks_survived = end_tick.saturating_sub(self.game_start_tick);
                let exit_reason = match player.status {
                    Status::Disconnected => PostGameExitReason::Disconnected,
                    Status::Dead | Status::Alive => PostGameExitReason::Shot,
                };
                PostGameLeaderboardEntry {
                    username: player.name.clone(),
                    color: player.color,
                    ticks_survived,
                    exit_reason,
                }
            })
            .collect::<Vec<_>>();

        entries.sort_by(|a, b| b.ticks_survived.cmp(&a.ticks_survived));

        // Handle timer expiration deaths.
        if let Some(timer_tick) = self.timer_expiration_tick {
            for entry in &mut entries {
                if let Some(player) = self.players.iter().find(|p| p.name == entry.username) {
                    if player.exit_tick == Some(timer_tick) {
                        if self.is_solo_mode {
                            let escaped = self
                                .maze
                                .is_outside(player.state.position.x, player.state.position.z);
                            entry.exit_reason = if escaped {
                                PostGameExitReason::Escaped
                            } else {
                                PostGameExitReason::Minotaured
                            };
                        } else {
                            entry.exit_reason = PostGameExitReason::Minotaured;
                        }
                    }
                }
            }
        }

        // Mark the winner if one was determined.
        if let Some(winner_idx) = self.winner_index {
            if let Some(winner_player) = self.players.get(winner_idx) {
                if let Some(winner_entry) = entries
                    .iter_mut()
                    .find(|e| e.username == winner_player.name)
                {
                    winner_entry.exit_reason = PostGameExitReason::Winner;
                }
            }
        }

        // Re-sort to ensure the winner is always first.
        entries.sort_by(|a, b| {
            let a_is_winner = matches!(a.exit_reason, PostGameExitReason::Winner);
            let b_is_winner = matches!(b.exit_reason, PostGameExitReason::Winner);
            match (a_is_winner, b_is_winner) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.ticks_survived.cmp(&a.ticks_survived),
            }
        });

        entries
    }
}

pub struct NetStats {
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

    pub fn log_if_ready(&mut self) {
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
        self.lobby.host_client_id.or(self.host_id)
    }
    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        let was_host = self.host_id() == Some(client_id);
        self.lobby.remove_client(client_id, network);
        if was_host {
            if let Some(new_host_id) = self.lobby.host_client_id {
                let message = ServerMessage::BeginDifficultySelection;
                let payload = encode_to_vec(&message, standard())
                    .expect("failed to serialize BeginDifficultySelection");
                network.send_message(new_host_id, AppChannel::ReliableOrdered, payload);
                println!(
                    "Host disconnected during difficulty selection. Client {} promoted.",
                    new_host_id
                );
            }
        }
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
            println!("All players disconnected during countdown. Server exiting.");
            std::process::exit(0);
        }
        if host_was_removed || no_host {
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
    pub player_colors: HashMap<u64, Color>,
    pending_usernames: HashSet<u64>,
    host_client_id: Option<u64>,
    pub lobby_timer_end: Option<f64>,
    pub one_minute_warning_sent: bool,
}

fn notify_new_host(network: &mut dyn ServerNetworkHandle, id: u64) {
    let message = ServerMessage::AppointHost;
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize AppointHost");
    network.send_message(id, AppChannel::ReliableOrdered, payload);
}

impl Lobby {
    pub fn new() -> Self {
        Self {
            pending_usernames: HashSet::new(),
            usernames: HashMap::new(),
            player_colors: HashMap::new(),
            host_client_id: None,
            lobby_timer_end: None,
            one_minute_warning_sent: false,
        }
    }

    pub fn set_host(&mut self, id: u64, network: &mut dyn ServerNetworkHandle) {
        self.host_client_id = Some(id);
        if self.lobby_timer_end.is_none() {
            self.lobby_timer_end = Some(
                common::time::now_as_secs_f64()
                    + common::constants::LOBBY_TIMER_DURATION.as_secs_f64(),
            );
        }
        notify_new_host(network, id);
    }

    pub fn is_host(&self, client_id: u64) -> bool {
        match self.host_client_id {
            Some(host_id) => host_id == client_id,
            None => false,
        }
    }

    pub fn register_connection(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        let is_first = self.pending_usernames.is_empty() && self.usernames.is_empty();
        self.pending_usernames.insert(client_id);
        if is_first {
            self.start_timer(network);
        }
    }

    fn start_timer(&mut self, network: &mut dyn ServerNetworkHandle) {
        if self.lobby_timer_end.is_none() {
            let end_time = common::time::now_as_secs_f64()
                + common::constants::LOBBY_TIMER_DURATION.as_secs_f64();
            self.lobby_timer_end = Some(end_time);
            let timer_msg = ServerMessage::LobbyTimer { end_time };
            let payload =
                encode_to_vec(&timer_msg, standard()).expect("failed to serialize LobbyTimer");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);
        }
    }

    pub fn remove_client(&mut self, client_id: u64, network: &mut dyn ServerNetworkHandle) {
        self.pending_usernames.remove(&client_id);
        self.player_colors.remove(&client_id);

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

        if self.usernames.is_empty() && self.pending_usernames.is_empty() {
            println!("All clients have disconnected. Server exiting...");
            std::process::exit(0);
        }
    }

    pub fn needs_username(&self, client_id: u64) -> bool {
        self.pending_usernames.contains(&client_id)
    }

    pub fn register_username(&mut self, client_id: u64, username: &str) -> Option<&str> {
        if self.pending_usernames.remove(&client_id) {
            self.usernames.insert(client_id, username.to_string());
            self.assign_color(client_id);
        }
        self.usernames.get(&client_id).map(|s| s.as_str())
    }

    pub fn username(&self, client_id: u64) -> Option<&str> {
        self.usernames.get(&client_id).map(|s| s.as_str())
    }

    pub fn color(&self, client_id: u64) -> Option<Color> {
        self.player_colors.get(&client_id).copied()
    }

    pub fn colors(&self) -> &HashMap<u64, Color> {
        &self.player_colors
    }

    pub fn roster_except(&self, client_id: u64) -> Vec<PlayerRosterEntry> {
        self.usernames
            .iter()
            .filter_map(|(&id, name)| {
                if id == client_id {
                    return None;
                }
                let color = self.player_colors.get(&id).copied().unwrap_or(COLORS[0]);
                Some(PlayerRosterEntry {
                    username: name.clone(),
                    color,
                })
            })
            .collect()
    }

    fn assign_color(&mut self, client_id: u64) -> Color {
        if let Some(color) = self.player_colors.get(&client_id).copied() {
            return color;
        }

        let available_colors: Vec<Color> = COLORS
            .iter()
            .copied()
            .filter(|&candidate| !self.player_colors.values().any(|&used| used == candidate))
            .collect();

        let color = if available_colors.is_empty() {
            COLORS[self.player_colors.len() % COLORS.len()]
        } else {
            available_colors[rng().random_range(0..available_colors.len())]
        };

        self.player_colors.insert(client_id, color);
        color
    }

    pub fn is_username_taken(&self, username: &str) -> bool {
        self.usernames.values().any(|existing| existing == username)
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
        self.pending_usernames.iter().cloned().collect()
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
        let colors = HashMap::new();
        let game_data = InitialData::new(&usernames, &colors, 1);
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
    fn register_connection_puts_client_in_pending_usernames() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(42);
        state.register_connection(42, &mut network);

        assert!(state.needs_username(42));
    }

    #[test]
    fn remove_client_clears_pending_state() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(99);
        state.register_connection(99, &mut network);

        state.remove_client(99, &mut network);

        assert!(!state.needs_username(99));
        assert!(state.username(99).is_none());
    }

    #[test]
    fn register_username_adds_user_and_removes_pending() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(5);
        state.register_connection(5, &mut network);

        state
            .register_username(5, "playerone")
            .expect("expected username to register");

        assert!(!state.needs_username(5));
        assert_eq!(state.username(5), Some("playerone"));
    }

    #[test]
    fn username_taken_checks_existing_names() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(10);
        state.register_connection(10, &mut network);
        state.register_username(10, "playerone");

        assert!(state.is_username_taken("playerone"));
        assert!(!state.is_username_taken("someoneelse"));
    }

    #[test]
    fn username_rejection_is_case_insensitive() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        network.add_client(10);
        state.register_connection(10, &mut network);
        state.register_username(10, "playerone");

        assert!(state.is_username_taken("playerone"));
        assert!(!state.is_username_taken("PLAYERONE"));
        assert!(!state.is_username_taken("PlayerOne"));

        assert!(!state.is_username_taken("player_two"));
        assert!(!state.is_username_taken("someoneelse"));
    }

    #[test]
    fn username_sanitization_enforces_case_insensitive_storage() {
        use common::player::sanitize_username;

        assert_eq!(sanitize_username("PlayerOne"), Ok("playerone".to_string()));
        assert_eq!(sanitize_username("PLAYERONE"), Ok("playerone".to_string()));
        assert_eq!(sanitize_username("playerone"), Ok("playerone".to_string()));
        assert_eq!(sanitize_username("pLaYeRoNe"), Ok("playerone".to_string()));

        assert_eq!(sanitize_username("PlayerTwo"), Ok("playertwo".to_string()));
        assert_ne!(
            sanitize_username("PlayerOne"),
            sanitize_username("PlayerTwo")
        );
    }

    #[test]
    fn usernames_except_excludes_requested_client() {
        let mut state = Lobby::new();
        let mut network = MockServerNetwork::new();
        for (id, name) in [(1, "alpha"), (2, "beta"), (3, "gamma")] {
            network.add_client(id);
            state.register_connection(id, &mut network);
            state.register_username(id, name);
        }

        let mut others = state.usernames_except(2);
        others.sort();
        assert_eq!(others, vec!["alpha".to_string(), "gamma".to_string()]);
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
        let colors = HashMap::new();
        let game_data = InitialData::new(&usernames, &colors, 1);

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
