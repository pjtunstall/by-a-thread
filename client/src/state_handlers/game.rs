use crate::{net::NetworkHandle, session::ClientSession, state::ClientState, ui::ClientUi};

pub fn handle(
    session: &mut ClientSession,
    _ui: &mut dyn ClientUi,
    _network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::InGame { .. }) {
        panic!(
            "called game::handle() when state was not InGame; current state: {:?}",
            session.state()
        );
    }

    None
}
