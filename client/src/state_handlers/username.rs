use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    net::NetworkHandle,
    session::{ClientSession, username_prompt, validate_username_input},
    state::ClientState,
    ui::ClientUi,
};
use shared::{net::AppChannel, protocol::ClientMessage};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    _network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        panic!(
            "called username::handle() when state was not ChoosingUsername; current state: {:?}",
            session.state()
        );
    }

    if let Some(input) = session.take_input() {
        if let ClientState::ChoosingUsername { prompt_printed } = session.state_mut() {
            let trimmed_input = input.trim();

            if trimmed_input.is_empty() {
                ui.show_sanitized_error("Username must not be empty.");
                *prompt_printed = false;
                return None;
            }

            let validation = validate_username_input(&input);
            match validation {
                Ok(username) => {
                    let message = ClientMessage::SetUsername(username);
                    let payload = encode_to_vec(&message, standard())
                        .expect("failed to serialize SetUsername");

                    _network.send_message(AppChannel::ReliableOrdered, payload);
                    ui.show_status_line("Waiting for server...");

                    return Some(ClientState::AwaitingUsernameConfirmation);
                }
                Err(err) => {
                    let message = err.to_string();
                    ui.show_sanitized_error(&message);
                    *prompt_printed = false;
                }
            }
        }
    }

    if let ClientState::ChoosingUsername { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            ui.show_sanitized_prompt(&username_prompt());
            *prompt_printed = true;
        }
    }

    if _network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "Disconnected while choosing username: {}.",
                _network.get_disconnect_reason()
            ),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use shared::player::MAX_USERNAME_LENGTH;

    fn set_choosing_username(session: &mut ClientSession) {
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
        });
    }

    mod guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called username::handle() when state was not ChoosingUsername; current state: Startup"
        )]
        fn choosing_username_panics_if_not_in_choosing_username_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn choosing_username_does_not_panic_in_choosing_username_state() {
            let mut session = ClientSession::new(0);
            set_choosing_username(&mut session);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn enforces_max_username_length() {
        let mut session = ClientSession::new(0);
        set_choosing_username(&mut session);

        let long_name = "A".repeat(MAX_USERNAME_LENGTH as usize + 1);
        session.add_input(long_name.clone());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(network.sent_messages.len(), 0);
        assert_eq!(ui.errors.len(), 1);
        assert!(
            ui.errors[0].contains("too long"),
            "UI error message did not contain 'too long'."
        );

        if let ClientState::ChoosingUsername { prompt_printed } = session.state() {
            assert_eq!(
                *prompt_printed, false,
                "'prompt_printed' flag was not reset after validation failure"
            );
        } else {
            panic!("state unexpectedly changed");
        }
    }

    #[test]
    fn handles_local_validation_error() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
        });

        session.add_input("   ".to_string());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(network.sent_messages.len(), 0);
        assert_eq!(ui.errors.len(), 1);
        assert!(ui.errors[0].contains("Username must not be empty"));

        if let ClientState::ChoosingUsername { prompt_printed } = session.state() {
            assert_eq!(*prompt_printed, false);
        } else {
            panic!("State unexpectedly changed");
        }
    }
}
