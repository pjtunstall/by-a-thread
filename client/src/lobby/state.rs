use shared::{auth::Passcode, snapshot::Snapshot};

#[derive(Debug)]
pub enum Lobby {
    Startup {
        prompt_printed: bool,
    },
    Connecting {
        pending_passcode: Option<Passcode>,
    },
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
        waiting_for_server: bool,
    },
    ChoosingUsername {
        prompt_printed: bool,
    },
    AwaitingUsernameConfirmation,
    Chat {
        awaiting_initial_roster: bool,
        waiting_for_server: bool,
    },
    Countdown {
        end_time: f64,
        snapshot: Snapshot,
    },
    ChoosingDifficulty {
        prompt_printed: bool,
        choice_sent: bool,
    },
}
