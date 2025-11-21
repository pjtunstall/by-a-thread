use super::auth::{parse_passcode_input, passcode_prompt};
use crate::{session::ClientSession, state::ClientState, ui::ClientUi};
use shared::auth::MAX_ATTEMPTS;

pub fn handle(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Startup { .. }) {
        panic!(
            "called startup::handle() when state was not Startup; current state: {:?}",
            session.state()
        );
    }

    if session.has_first_passcode() {
        return Some(ClientState::Connecting);
    }

    if let Some(input_string) = session.take_input() {
        if let Some(passcode) = parse_passcode_input(&input_string) {
            session.store_first_passcode(passcode);
            return Some(ClientState::Connecting);
        } else {
            ui.show_sanitized_error(&format!(
                "Invalid format: \"{}\". Passcode must be a 6-digit number.",
                input_string.trim()
            ));

            ui.show_sanitized_prompt(&passcode_prompt(MAX_ATTEMPTS));

            if let ClientState::Startup { prompt_printed } = session.state_mut() {
                *prompt_printed = true;
            }
            return None;
        }
    }

    let needs_prompt = match session.state() {
        ClientState::Startup { prompt_printed } => !prompt_printed,
        _ => false,
    };

    if needs_prompt {
        ui.show_prompt(&passcode_prompt(MAX_ATTEMPTS));

        if let ClientState::Startup { prompt_printed } = session.state_mut() {
            *prompt_printed = true;
        }
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockUi;

    mod guards {
        use super::*;

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
    fn transitions_to_connecting_if_passcode_is_present() {
        let mut session = ClientSession::new(0);
        let passcode = shared::auth::Passcode {
            bytes: vec![1, 2, 3, 4, 5, 6],
            string: "123456".to_string(),
        };
        session.store_first_passcode(passcode);

        let mut ui = MockUi::default();

        let next = handle(&mut session, &mut ui);

        assert!(ui.prompts.is_empty());
        assert!(matches!(next, Some(ClientState::Connecting)));
    }

    #[test]
    fn transitions_to_connecting_after_first_prompt() {
        let mut session = ClientSession::new(0);
        session.add_input("123456".to_string());

        let mut ui = MockUi::default();

        let next = handle(&mut session, &mut ui);

        assert!(ui.prompts.is_empty());
        assert!(matches!(next, Some(ClientState::Connecting)));
        assert_eq!(session.take_first_passcode().unwrap().string, "123456");
    }

    #[test]
    fn handles_invalid_input_and_reprompts() {
        let mut session = ClientSession::new(0);
        session.add_input("abc".to_string());

        let mut ui = MockUi::default();

        let next = handle(&mut session, &mut ui);

        assert!(next.is_none());
        assert_eq!(ui.errors.len(), 1);
        assert_eq!(ui.prompts.len(), 1, "Should show one prompt for the retry");

        ui.errors.clear();
        ui.prompts.clear();

        let next_2 = handle(&mut session, &mut ui);

        assert!(next_2.is_none());
        assert!(
            ui.prompts.is_empty(),
            "Should not show a second prompt on the next frame"
        );
        assert!(ui.errors.is_empty());

        if let ClientState::Startup { prompt_printed } = session.state() {
            assert!(*prompt_printed);
        } else {
            panic!("Expected Startup state");
        }
    }
}
