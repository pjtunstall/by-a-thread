use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    session::ClientSession,
    state::ClientState,
    {net::NetworkHandle, ui::ClientUi},
};
use shared::{
    auth::{MAX_ATTEMPTS, Passcode},
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Authenticating { .. }) {
        panic!(
            "called auth::handle() when state was not Authenticating; current state: {:?}",
            session.state()
        );
    }

    let mut guess_sent_this_frame = false;

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                session.set_auth_waiting_for_server(false);
                session.clear_status_line();
                if message.starts_with("The game has already started.") {
                    ui.show_message(&message);
                    return Some(ClientState::TransitioningToDisconnected { message });
                }

                ui.show_sanitized_message(&format!("Server: {}", message));

                if message.starts_with("Authentication successful!") {
                    return Some(ClientState::ChoosingUsername {
                        prompt_printed: false,
                    });
                } else if message.starts_with("Incorrect passcode. Try again.") {
                    if let ClientState::Authenticating {
                        waiting_for_input,
                        guesses_left,
                        ..
                    } = session.state_mut()
                    {
                        *guesses_left = guesses_left.saturating_sub(1);
                        ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
                        *waiting_for_input = true;
                    }
                } else if message.starts_with("Incorrect passcode. Disconnecting.") {
                    return Some(ClientState::TransitioningToDisconnected {
                        message: "Authentication failed.".to_string(),
                    });
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    let mut input_to_process = Vec::new();
    std::mem::swap(&mut session.input_queue, &mut input_to_process);

    for input_string in input_to_process {
        let mut should_mark_waiting_for_server = false;
        if let ClientState::Authenticating {
            waiting_for_input,
            guesses_left,
        } = session.state_mut()
        {
            if *waiting_for_input {
                if let Some(passcode) = parse_passcode_input(&input_string) {
                    ui.show_sanitized_message("Sending new guess...");

                    let message = ClientMessage::SendPasscode(passcode.bytes);
                    let payload = encode_to_vec(&message, standard())
                        .expect("failed to serialize SendPasscode");
                    network.send_message(AppChannel::ReliableOrdered, payload);

                    should_mark_waiting_for_server = true;
                    *waiting_for_input = false;
                    guess_sent_this_frame = true;
                } else {
                    ui.show_sanitized_error(&format!(
                        "Invalid format: \"{}\". Passcode must be a 6-digit number.",
                        input_string.trim()
                    ));

                    ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
                }
            }
        }
        if !matches!(session.state(), ClientState::Authenticating { .. }) {
            break;
        }

        if should_mark_waiting_for_server {
            session.set_auth_waiting_for_server(true);
        }
    }

    let waiting_for_server = session.auth_waiting_for_server;
    if let ClientState::Authenticating {
        waiting_for_input, ..
    } = session.state_mut()
    {
        if guess_sent_this_frame {
            *waiting_for_input = false;
        } else if !*waiting_for_input && !waiting_for_server {
            *waiting_for_input = true;
        }
    }

    if network.is_disconnected() {
        let reason = network.get_disconnect_reason();

        return Some(ClientState::TransitioningToDisconnected {
            message: format!("Disconnected while authenticating: {}.", reason),
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
