use super::auth::{parse_passcode_input, passcode_prompt};
use crate::{
    state::{ClientSession, ClientState},
    ui::{ClientUi, UiInputError},
};
use shared::{auth::MAX_ATTEMPTS, chat::MAX_CHAT_MESSAGE_BYTES};

pub fn handle(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Startup { .. }) {
        panic!(
            "called startup::handle() when state was not Startup; current state: {:?}",
            session.state()
        );
    }

    match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
        Ok(Some(input_string)) => {
            if let Some(passcode) = parse_passcode_input(&input_string) {
                session.store_first_passcode(passcode);
                Some(ClientState::Connecting)
            } else {
                ui.show_sanitized_error("Invalid format. Please enter a 6-digit number.");
                ui.show_sanitized_prompt(&passcode_prompt(MAX_ATTEMPTS));
                None
            }
        }
        Ok(None) => None,
        Err(UiInputError::Disconnected) => Some(ClientState::Disconnected {
            message: "input thread disconnected.".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockUi;

    mod guards {
        use super::*;
        use crate::state::{ClientSession, ClientState};

        #[test]
        #[should_panic(
            expected = "called startup::handle() when state was not Startup; current state: Connecting"
        )]
        fn startup_panics_if_not_in_startup_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();

            handle(&mut session, &mut ui);
        }

        #[test]
        fn startup_does_not_panic_in_startup_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            assert!(
                handle(&mut session, &mut ui).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn reprompts_after_invalid_passcode() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Ok(Some("abc".into()))]);

        assert!(handle(&mut session, &mut ui).is_none());
        assert_eq!(
            ui.errors,
            vec!["Invalid format. Please enter a 6-digit number.".to_string()]
        );
        assert_eq!(ui.prompts.len(), 1);
    }

    #[test]
    fn returns_disconnected_when_input_thread_stops() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Err(UiInputError::Disconnected)]);

        let next = handle(&mut session, &mut ui);
        match next {
            Some(ClientState::Disconnected { message }) => {
                assert_eq!(message, "input thread disconnected.");
            }
            _ => panic!("unexpected transition: expected disconnection"),
        }
    }

    #[test]
    fn prompts_only_once_when_waiting_for_input() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::default();

        assert!(handle(&mut session, &mut ui).is_none());
        assert!(ui.prompts.is_empty());

        ui.messages.clear();
        ui.errors.clear();

        assert!(handle(&mut session, &mut ui).is_none());
        assert!(ui.prompts.is_empty());
    }

    #[test]
    fn returns_connecting_when_valid_passcode_received() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Ok(Some("123456".into()))]);

        let next = handle(&mut session, &mut ui);
        assert!(matches!(next, Some(ClientState::Connecting)));
        assert_eq!(session.take_first_passcode().unwrap().string, "123456");
    }
}
