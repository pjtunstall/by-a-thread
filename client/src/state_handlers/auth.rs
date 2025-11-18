use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use super::{parse_passcode_input, passcode_prompt};
use crate::state::{ClientSession, ClientState};
use crate::{
    net::NetworkHandle,
    ui::{ClientUi, UiInputError},
};
use shared::net::AppChannel;
use shared::{
    chat::MAX_CHAT_MESSAGE_BYTES,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Authenticating { .. }) {
        panic!(
            "called authenticating() when state was not Authenticating; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));

                if message.starts_with("Authentication successful!") {
                    return Some(ClientState::ChoosingUsername {
                        prompt_printed: false,
                        awaiting_confirmation: false,
                    });
                } else if message.starts_with("Incorrect passcode. Try again.") {
                    if let ClientState::Authenticating {
                        waiting_for_input,
                        guesses_left,
                    } = session.state_mut()
                    {
                        *guesses_left = guesses_left.saturating_sub(1);
                        ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
                        *waiting_for_input = true;
                    }
                } else if message.starts_with("Incorrect passcode. Disconnecting.") {
                    return Some(ClientState::Disconnected {
                        message: "Authentication failed.".to_string(),
                    });
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    if let ClientState::Authenticating {
        waiting_for_input,
        guesses_left,
    } = session.state_mut()
    {
        if *waiting_for_input {
            match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
                Ok(Some(input_string)) => {
                    if let Some(passcode) = parse_passcode_input(&input_string) {
                        ui.show_sanitized_message("Sending new guess...");

                        let message = ClientMessage::SendPasscode(passcode.bytes);
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize SendPasscode");
                        network.send_message(AppChannel::ReliableOrdered, payload);

                        *waiting_for_input = false;
                    } else {
                        ui.show_sanitized_error(&format!(
                            "Invalid format: {}. Please enter a 6-digit number.",
                            input_string
                        ));
                        ui.show_sanitized_message(
                            &format!(
                                "Please type a new 6-digit passcode and press Enter. ({} guesses remaining.)",
                                *guesses_left
                            ),
                        );
                    }
                }
                Ok(None) => {}
                Err(UiInputError::Disconnected) => {
                    return Some(ClientState::Disconnected {
                        message: "input thread disconnected.".to_string(),
                    });
                }
            }
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected while authenticating: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}
