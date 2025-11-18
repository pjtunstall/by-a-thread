use super::{parse_passcode_input, passcode_prompt};
use crate::{
    state::{ClientSession, ClientState, MAX_ATTEMPTS},
    ui::{ClientUi, UiInputError},
};
use shared::chat::MAX_CHAT_MESSAGE_BYTES;

pub fn handle(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Startup { .. }) {
        panic!(
            "called startup() when state was not Startup; current state: {:?}",
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
