use crate::{
    net::NetworkHandle,
    state::{ClientSession, ClientState},
    ui::ClientUi,
};
use shared::auth::MAX_ATTEMPTS;

pub fn handle(
    session: &mut ClientSession,
    _ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Connecting) {
        panic!(
            "called connecting::handle() when state was not Connecting; current state: {:?}",
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

#[cfg(test)]
mod tests {
    use super::*;

    mod guards {
        use super::*;
        use crate::{
            state::{ClientSession, ClientState},
            test_helpers::MockNetwork,
            test_helpers::MockUi,
        };

        #[test]
        #[should_panic(
            expected = "called connecting::handle() when state was not Connecting; current state: Startup"
        )]
        fn connecting_panics_if_not_in_connecting_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn connecting_does_not_panic_in_connecting_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }
}
