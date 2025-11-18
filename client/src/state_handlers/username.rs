use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState, username_prompt, validate_username_input};
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
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        panic!(
            "called choosing_username() when state was not ChoosingUsername; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::Welcome { username }, _)) => {
                ui.show_sanitized_message(&format!("Welcome, {}!", username));
                return Some(ClientState::InChat);
            }
            Ok((ServerMessage::UsernameError { message }, _)) => {
                ui.show_sanitized_error(&format!("Username error: {}", message));
                ui.show_sanitized_message("Please try a different username.");
                if let ClientState::ChoosingUsername {
                    prompt_printed,
                    awaiting_confirmation,
                } = session.state_mut()
                {
                    *awaiting_confirmation = false;
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    if let ClientState::ChoosingUsername {
        prompt_printed,
        awaiting_confirmation,
    } = session.state_mut()
    {
        if !*awaiting_confirmation {
            if !*prompt_printed {
                ui.show_sanitized_prompt(&username_prompt());
                *prompt_printed = true;
            }

            match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
                Ok(Some(input)) => {
                    let validation = validate_username_input(&input);
                    match validation {
                        Ok(username) => {
                            let message = ClientMessage::SetUsername(username);
                            let payload = encode_to_vec(&message, standard())
                                .expect("failed to serialize SetUsername");
                            network.send_message(AppChannel::ReliableOrdered, payload);

                            *awaiting_confirmation = true;
                        }
                        Err(err) => {
                            let message = err.to_string();
                            ui.show_sanitized_error(&message);
                            *prompt_printed = false;
                        }
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
                "Disconnected while choosing username: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}
