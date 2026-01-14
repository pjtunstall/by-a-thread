use super::auth::{parse_passcode_input, passcode_prompt};
use crate::{
    lobby::ui::{LobbyUi, UiErrorKind},
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::auth::MAX_ATTEMPTS;

pub fn handle(session: &mut ClientSession, ui: &mut dyn LobbyUi) -> Option<ClientState> {
    if !matches!(&session.state, ClientState::Lobby(Lobby::Passcode { .. })) {
        panic!(
            "called passcode::handle() when state was not Passcode; current state: {:?}",
            &session.state
        );
    }

    if let Some(input_string) = session.take_input() {
        if !input_string.trim().is_empty() {
            if let Some(passcode) = parse_passcode_input(&input_string) {
                return Some(ClientState::Lobby(Lobby::Connecting {
                    pending_passcode: Some(passcode),
                }));
            } else {
                ui.show_typed_error(
                    UiErrorKind::PasscodeFormat,
                    &format!(
                        "Invalid format: \"{}\". Passcode must be a 6-digit number.",
                        input_string.trim()
                    ),
                );

                ui.show_sanitized_prompt(&passcode_prompt(MAX_ATTEMPTS));

            if let ClientState::Lobby(Lobby::Passcode { prompt_printed }) = &mut session.state {
                *prompt_printed = true;
            }
            return None;
        }
    }
    }

    let needs_prompt = match &session.state {
        ClientState::Lobby(Lobby::Passcode { prompt_printed }) => !prompt_printed,
        _ => false,
    };

    if needs_prompt {
        ui.show_prompt(&passcode_prompt(MAX_ATTEMPTS));

        if let ClientState::Lobby(Lobby::Passcode { prompt_printed }) = &mut session.state {
            *prompt_printed = true;
        }
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lobby::ui::UiErrorKind, test_helpers::MockUi};

    mod guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called passcode::handle() when state was not Passcode; current state: Lobby(Connecting { pending_passcode: None })"
        )]
        fn passcode_panics_if_not_in_passcode_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Lobby(Lobby::Connecting {
                pending_passcode: None,
            }));
            let mut ui = MockUi::default();

            handle(&mut session, &mut ui);
        }

        #[test]
        fn passcode_does_not_panic_in_passcode_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Lobby(Lobby::Passcode {
                prompt_printed: false,
            }));
            let mut ui = MockUi::default();
            assert!(
                handle(&mut session, &mut ui).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn transitions_to_connecting_after_first_prompt() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Passcode {
            prompt_printed: false,
        }));
        session.add_input("123456".to_string());

        let mut ui = MockUi::default();

        let next = handle(&mut session, &mut ui);

        assert!(ui.prompts.is_empty());
        match next {
            Some(ClientState::Lobby(Lobby::Connecting {
                pending_passcode: Some(passcode),
            })) => assert_eq!(passcode.string, "123456"),
            other => panic!("unexpected next state: {:?}", other),
        }
    }

    #[test]
    fn handles_invalid_input_and_reprompts() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Passcode {
            prompt_printed: false,
        }));
        session.add_input("abc".to_string());

        let mut ui = MockUi::default();

        let next = handle(&mut session, &mut ui);

        assert!(next.is_none());
        assert_eq!(ui.error_kinds, vec![UiErrorKind::PasscodeFormat]);
        assert_eq!(ui.prompts.len(), 1, "should show one prompt for the retry");

        ui.errors.clear();
        ui.error_kinds.clear();
        ui.prompts.clear();

        let next_2 = handle(&mut session, &mut ui);

        assert!(next_2.is_none());
        assert!(
            ui.prompts.is_empty(),
            "should not show a second prompt on the next frame"
        );
        assert!(ui.errors.is_empty());

        match &session.state {
            ClientState::Lobby(Lobby::Passcode { prompt_printed }) => assert!(*prompt_printed),
            _ => panic!("expected Passcode state"),
        }
    }
}
