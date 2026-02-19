use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use crate::{
    frame::FrameRate,
    lobby::state::Lobby,
    post_game_chat::PostGameChat,
    state::{ClientState, InputMode},
};
use common::player::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username};

#[derive(Debug)]
pub struct ClockSample {
    pub server_time: f64,
    pub client_receive_time: f64,
    pub rtt: f64,
}

#[derive(Debug)]
pub struct ClientSession {
    pub client_id: u64,
    pub is_host: bool,
    pub state: ClientState,
    pub clock: Clock,
    pub input_queue: Vec<String>,
    pub local_player_index: Option<usize>,
    pub disconnected_notified: bool,
    pub pending_disconnect: Option<String>,
    pub server_addr: Option<SocketAddr>,
    waiting_since: Option<Instant>,
    waiting_message_shown: bool,
    pub lobby_timer_end: Option<f64>,
}

impl ClientSession {
    pub fn new(client_id: u64, server_addr: Option<SocketAddr>) -> Self {
        Self {
            client_id,
            is_host: false,
            state: ClientState::Lobby(Lobby::ServerAddress {
                prompt_printed: false,
            }),
            clock: Clock::new(),
            input_queue: Vec::new(),
            local_player_index: None,
            disconnected_notified: false,
            pending_disconnect: None,
            server_addr,
            waiting_since: None,
            waiting_message_shown: false,
            lobby_timer_end: None,
        }
    }

    pub fn transition(&mut self, new_state: ClientState) {
        if matches!(
            new_state,
            ClientState::Disconnected { .. } | ClientState::EndAfterLeaderboard
        ) {
            self.disconnected_notified = false;
            self.pending_disconnect = None;
        }
        if matches!(new_state, ClientState::PostGameChat { .. }) {
            self.input_queue.clear();
        }
        if matches!(new_state, ClientState::Lobby(Lobby::Countdown { .. })) {
            self.lobby_timer_end = None;
        }
        self.state = new_state;
    }

    pub fn with_choosing_username<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut bool) -> R,
    {
        match &mut self.state {
            ClientState::Lobby(Lobby::ChoosingUsername { prompt_printed }) => {
                Some(f(prompt_printed))
            }
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
        matches!(&self.state, ClientState::Lobby(Lobby::Countdown { .. }))
    }

    pub fn is_countdown_finished(&self) -> bool {
        matches!(&self.state, ClientState::Lobby(Lobby::Countdown { end_time, .. }) if self.clock.estimated_server_time >= *end_time)
    }

    pub fn set_chat_waiting_for_server(&mut self, waiting: bool) {
        match &mut self.state {
            ClientState::Lobby(Lobby::Chat {
                waiting_for_server, ..
            })
            | ClientState::PostGameChat(PostGameChat {
                waiting_for_server, ..
            }) => {
                *waiting_for_server = waiting;
            }
            _ => {}
        }
    }

    pub fn chat_waiting_for_server(&self) -> bool {
        matches!(
            self.state,
            ClientState::Lobby(Lobby::Chat {
                waiting_for_server: true,
                ..
            }) | ClientState::PostGameChat(PostGameChat {
                waiting_for_server: true,
                ..
            })
        )
    }

    pub fn expect_initial_roster(&mut self) {
        match &mut self.state {
            ClientState::Lobby(Lobby::Chat {
                awaiting_initial_roster,
                ..
            })
            | ClientState::PostGameChat(PostGameChat {
                awaiting_initial_roster,
                ..
            }) => {
                *awaiting_initial_roster = true;
            }
            _ => {}
        }
    }

    pub fn awaiting_initial_roster(&self) -> bool {
        matches!(
            self.state,
            ClientState::Lobby(Lobby::Chat {
                awaiting_initial_roster: true,
                ..
            }) | ClientState::PostGameChat(PostGameChat {
                awaiting_initial_roster: true,
                ..
            })
        )
    }

    pub fn mark_initial_roster_received(&mut self) {
        match &mut self.state {
            ClientState::Lobby(Lobby::Chat {
                awaiting_initial_roster,
                ..
            })
            | ClientState::PostGameChat(PostGameChat {
                awaiting_initial_roster,
                ..
            }) => {
                *awaiting_initial_roster = false;
            }
            _ => {}
        }
    }

    pub fn input_mode(&self) -> InputMode {
        match &self.state {
            ClientState::Lobby(Lobby::ServerAddress { .. }) => InputMode::Enabled,
            ClientState::Lobby(Lobby::MatchmakerMenu { .. }) => InputMode::SingleKey,
            ClientState::Lobby(Lobby::Connecting { .. }) => InputMode::Hidden,
            ClientState::Lobby(Lobby::ChoosingUsername { .. }) => InputMode::Enabled,
            ClientState::Lobby(Lobby::AwaitingUsernameConfirmation) => InputMode::DisabledWaiting,
            ClientState::Lobby(Lobby::Chat {
                waiting_for_server, ..
            }) => {
                if *waiting_for_server {
                    InputMode::DisabledWaiting
                } else {
                    InputMode::Enabled
                }
            }
            ClientState::PostGameChat(PostGameChat {
                waiting_for_server,
                leaderboard_received,
                ..
            }) => {
                if *leaderboard_received {
                    InputMode::Hidden
                } else if *waiting_for_server {
                    InputMode::DisabledWaiting
                } else {
                    InputMode::Enabled
                }
            }
            ClientState::Lobby(Lobby::ChoosingDifficulty { choice_sent, .. }) => {
                if *choice_sent {
                    InputMode::DisabledWaiting
                } else {
                    InputMode::SingleKey
                }
            }
            ClientState::Lobby(Lobby::Countdown { .. }) => InputMode::Hidden,
            ClientState::Disconnected { .. }
            | ClientState::EndAfterLeaderboard
            | ClientState::Transitioning => InputMode::Hidden,
            ClientState::Game(_) => InputMode::SingleKey,
        }
    }

    pub fn prepare_ui_state(&mut self) -> InputUiState {
        let waiting_active = matches!(self.input_mode(), InputMode::DisabledWaiting)
            || matches!(&self.state, ClientState::Lobby(Lobby::Connecting { .. }));

        if waiting_active {
            if self.waiting_since.is_none() {
                self.waiting_since = Some(Instant::now());
                self.waiting_message_shown = false;
            }
        } else {
            self.waiting_since = None;
            self.waiting_message_shown = false;
        }

        let show_waiting_message = waiting_active
            && !self.waiting_message_shown
            && self
                .waiting_since
                .map(|start| start.elapsed().as_millis() >= 300)
                .unwrap_or(false);

        if show_waiting_message {
            self.waiting_message_shown = true;
        }

        InputUiState {
            mode: self.input_mode(),
            show_waiting_message,
        }
    }

    pub fn set_pending_disconnect(&mut self, message: String) {
        if self.pending_disconnect.is_none() && !self.state.is_disconnected() {
            self.pending_disconnect = Some(message);
        }
    }

    pub fn take_pending_disconnect(&mut self) -> Option<String> {
        self.pending_disconnect.take()
    }
}

pub struct InputUiState {
    pub mode: InputMode,
    pub show_waiting_message: bool,
}

#[derive(Debug)]
pub struct Clock {
    pub estimated_server_time: f64,
    pub samples: VecDeque<ClockSample>,
    pub smoothed_rtt: f64,
    pub accumulated_time: f64,
    pub continuous_sim_time: f64,
    pub sim_tick: u64,
    pub fps: FrameRate,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            estimated_server_time: 0.0, // Seconds.
            samples: VecDeque::new(),
            smoothed_rtt: 0.0,
            accumulated_time: 0.0,
            continuous_sim_time: 0.0,
            sim_tick: 0,
            fps: FrameRate::default(),
        }
    }
}

pub fn username_prompt() -> String {
    format!(
        "Choose a username (1-{} characters, letters/numbers only): ",
        MAX_USERNAME_LENGTH
    )
}

pub fn validate_username_input(input: &str) -> Result<String, UsernameError> {
    sanitize_username(input)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use common::player::UsernameError;

    #[test]
    fn new_session_starts_in_server_address_state() {
        let session = ClientSession::new(0, crate::server_address::default_server_address().ok());
        assert!(matches!(
            session.state,
            ClientState::Lobby(Lobby::ServerAddress {
                prompt_printed: false
            })
        ));
        assert!(session.server_addr.is_some());
    }

    #[test]
    fn transition_updates_state() {
        let mut session =
            ClientSession::new(0, crate::server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        }));
        assert!(matches!(
            session.state,
            ClientState::Lobby(Lobby::Connecting { .. })
        ));
        session.transition(ClientState::Disconnected {
            message: "done".to_string(),
        });

        match session.state {
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
        let validated = validate_username_input("  Player1  ").expect("valid username expected");
        assert_eq!(validated, "player1");
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
        let mut session =
            ClientSession::new(0, crate::server_address::default_server_address().ok());

        session.add_input("message one".to_string());
        session.add_input("message two".to_string());

        assert_eq!(session.take_input(), Some("message one".to_string()));
        assert_eq!(session.take_input(), Some("message two".to_string()));
        assert_eq!(session.take_input(), None);
    }

    #[test]
    fn waiting_message_flags_after_delay() {
        let mut session =
            ClientSession::new(0, crate::server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: false,
            waiting_for_server: false,
        }));
        session.set_chat_waiting_for_server(true);

        let first_state = session.prepare_ui_state();
        assert!(!first_state.show_waiting_message);

        std::thread::sleep(Duration::from_millis(320));

        let second_state = session.prepare_ui_state();
        assert!(second_state.show_waiting_message);

        let third_state = session.prepare_ui_state();
        assert!(
            !third_state.show_waiting_message,
            "should only fire once per wait"
        );

        session.set_chat_waiting_for_server(false);
        let fourth_state = session.prepare_ui_state();
        assert!(!fourth_state.show_waiting_message);
        assert!(session.waiting_since.is_none());
    }

    #[test]
    fn waiting_message_triggers_during_connecting() {
        let mut session =
            ClientSession::new(0, crate::server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        }));

        let first_state = session.prepare_ui_state();
        assert!(!first_state.show_waiting_message);

        std::thread::sleep(Duration::from_millis(320));

        let second_state = session.prepare_ui_state();
        assert!(second_state.show_waiting_message);
    }
}
