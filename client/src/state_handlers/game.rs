use crate::{net::NetworkHandle, session::ClientSession, state::ClientState, ui::ClientUi};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    _network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::InGame { .. }) {
        panic!(
            "called game::handle() when state was not InGame; current state: {:?}",
            session.state()
        );
    }

    ui.show_sanitized_message("Exiting for now.");
    return Some(ClientState::TransitioningToDisconnected {
        message: "next state not yet implemented".to_string(),
    });
}
