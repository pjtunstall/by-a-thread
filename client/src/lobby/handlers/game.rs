use crate::{net::NetworkHandle, session::ClientSession, state::ClientState, lobby::ui::LobbyUi};

pub fn handle(
    session: &mut ClientSession,
    _ui: &mut dyn LobbyUi,
    _network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Game(_)) {
        panic!(
            "called game::handle() when state was not Game; current state: {:?}",
            session.state()
        );
    }

    None
}
