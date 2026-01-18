use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    auth::{MAX_ATTEMPTS, Passcode},
    input::sanitize,
    net::AppChannel,
    player::MAX_USERNAME_LENGTH,
    protocol::{
        AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE, AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE,
        ClientMessage, GAME_ALREADY_STARTED_MESSAGE, ServerMessage, auth_success_message,
    },
};

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let Lobby::Authenticating {
        waiting_for_input,
        guesses_left,
        waiting_for_server,
    } = lobby_state
    else {
        unreachable!();
    };

    let mut guess_sent_this_frame = false;

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                session.set_auth_waiting_for_server(false);
                *waiting_for_server = false;
                let sanitized_message = sanitize(&message);
                if sanitized_message == GAME_ALREADY_STARTED_MESSAGE {
                    return Some(ClientState::Disconnected {
                        message: sanitized_message,
                    });
                }

                ui.show_message(&format!("Server: {}", sanitized_message));

                if sanitized_message == auth_success_message(MAX_USERNAME_LENGTH) {
                    return Some(ClientState::Lobby(Lobby::ChoosingUsername {
                        prompt_printed: false,
                    }));
                } else if sanitized_message == AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE {
                    *guesses_left = guesses_left.saturating_sub(1);
                    ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
                    *waiting_for_input = true;
                } else if sanitized_message == AUTH_INCORRECT_PASSCODE_DISCONNECTING_MESSAGE {
                    return Some(ClientState::Disconnected {
                        message: "authentication failed".to_string(),
                    });
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[DESERIALIZATION ERROR: {}]", e),
            ),
        }
    }

    let mut input_to_process = Vec::new();
    std::mem::swap(&mut session.input_queue, &mut input_to_process);

    for input_string in input_to_process {
        let mut should_mark_waiting_for_server = false;
        if input_string.trim().is_empty() {
            continue;
        }
        if *waiting_for_input {
            if let Some(passcode) = parse_passcode_input(&input_string) {
                ui.show_sanitized_message("Sending new guess...");

                let message = ClientMessage::SendPasscode(passcode.bytes);
                let payload =
                    encode_to_vec(&message, standard()).expect("failed to serialize SendPasscode");
                network.send_message(AppChannel::ReliableOrdered, payload);

                should_mark_waiting_for_server = true;
                *waiting_for_server = true;
                *waiting_for_input = false;
                guess_sent_this_frame = true;
            } else {
                ui.show_typed_error(
                    UiErrorKind::PasscodeFormat,
                    &format!(
                        "Invalid format: \"{}\". Passcode must be a 6-digit number.",
                        input_string.trim()
                    ),
                );

                ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
            }
        }
        if should_mark_waiting_for_server {
            session.set_auth_waiting_for_server(true);
        }
    }

    if session.auth_waiting_for_server() && !*waiting_for_input && !guess_sent_this_frame {
        ui.show_prompt(&passcode_prompt(*guesses_left));
    }

    if guess_sent_this_frame {
        *waiting_for_input = false;
    } else if !*waiting_for_input && !session.auth_waiting_for_server() {
        *waiting_for_input = true;
    }

    if network.is_disconnected() {
        let reason = network.get_disconnect_reason();
        return Some(ClientState::Disconnected {
            message: format!("disconnected while authenticating: {}", reason),
        });
    }

    None
}

pub fn passcode_prompt(remaining: u8) -> String {
    if remaining == MAX_ATTEMPTS {
        format!("Enter passcode ({} guesses): ", remaining)
    } else {
        format!(
            "Please enter new 6-digit passcode. ({} guesses remaining): ",
            remaining
        )
    }
}

pub fn parse_passcode_input(input: &str) -> Option<Passcode> {
    let s = input.trim();
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_digit()) {
        let mut bytes = vec![0u8; 6];
        for (i, c) in s.chars().enumerate() {
            bytes[i] = c.to_digit(10).unwrap() as u8;
        }
        return Some(Passcode {
            bytes,
            string: s.to_string(),
        });
    }
    None
}
