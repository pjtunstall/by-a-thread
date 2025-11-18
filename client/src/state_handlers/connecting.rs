use crate::state::{ClientSession, ClientState, MAX_ATTEMPTS};
use crate::{net::NetworkHandle, ui::ClientUi};

pub fn handle(
    session: &mut ClientSession,
    _ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Connecting) {
        panic!(
            "called connecting() when state was not Connecting; current state: {:?}",
            session.state()
        );
    }

    if network.is_connected() {
        if session.has_first_passcode() {
            Some(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            })
        } else {
            Some(ClientState::Disconnected {
                message: "Internal error: No passcode to send.".to_string(),
            })
        }
    } else if network.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!("Connection failed: {}.", network.get_disconnect_reason()),
        })
    } else {
        None
    }
}
