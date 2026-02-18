pub mod chat;
pub mod connecting;
pub mod countdown;
pub mod difficulty;
pub mod server_address;
pub mod start_countdown;
pub mod username;
pub mod waiting;

use super::flow::LobbyStep;
use crate::{net::RenetNetworkHandle, run::ClientRunner};

pub fn update(runner: &mut ClientRunner) {
    let mut network_handle = RenetNetworkHandle::new(&mut runner.client, &mut runner.transport);
    let is_host = runner.session.is_host;

    match super::flow::update(
        &mut runner.session,
        &mut runner.ui,
        &mut network_handle,
        Some(&runner.assets),
        is_host,
    ) {
        LobbyStep::Continue => {}
        LobbyStep::StartGame => {
            // TODO: Decide whether to do anything with a returned error here.
            // If not, why return an error? Currently `runner.state_game` prints
            // an error message in the UI window.
            let _ = runner.start_game();
        }
        LobbyStep::Transition(new_state) => runner.session.transition(new_state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lobby::{state::Lobby, ui::LobbyUi},
        server_address,
        session::ClientSession,
        state::ClientState,
        test_helpers::{MockNetwork, MockUi},
    };
    use common::{input::sanitize, protocol::ServerMessage};

    #[test]
    fn client_banner_is_printed_correctly() {
        let mut ui = MockUi::default();
        let version = "0.1.0";
        let server_addr = server_address::default_server_address().ok().expect("test env");

        let expected_banner = format!(
            "Client Banner: Version={}, Server={}",
            version, server_addr
        );

        ui.print_client_banner(version, server_addr, None);

        assert_eq!(ui.messages, vec![expected_banner]);
        assert!(ui.errors.is_empty());
        assert!(ui.prompts.is_empty());
    }

    #[test]
    fn test_incoming_server_data_is_sanitized_before_display() {
        let bell = '\x07';
        let esc = '\x1B';

        let mut session_chat = ClientSession::new(0, server_address::default_server_address().ok());
        session_chat.transition(ClientState::Lobby(Lobby::Chat {
            awaiting_initial_roster: true,
            waiting_for_server: false,
        }));
        session_chat.mark_initial_roster_received();
        let mut ui_chat = MockUi::new();
        let mut network_chat = MockNetwork::new();

        let malicious_chat = ServerMessage::ChatMessage {
            username: format!("User{}", bell),
            color: common::player::Color::RED,
            content: format!("Hello{}World", esc),
        };
        network_chat.queue_server_message(malicious_chat);

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session_chat.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                chat::handle(
                    lobby_state,
                    &mut session_chat,
                    &mut ui_chat,
                    &mut network_chat,
                    None,
                )
            } else {
                panic!("expected Lobby state");
            };
            session_chat.state = temp_state;
            result
        };

        assert_eq!(
            ui_chat.messages.len(),
            1,
            "expected one chat message to be displayed"
        );
        assert_eq!(
            ui_chat.messages[0],
            sanitize("User\x07: Hello\x1BWorld"),
            "chat message was not sanitized"
        );

        let mut session_info = ClientSession::new(0, server_address::default_server_address().ok());
        session_info.transition(ClientState::Lobby(Lobby::ChoosingUsername {
            prompt_printed: true,
        }));
        let mut ui_info = MockUi::new();
        let mut network_info = MockNetwork::new();

        let malicious_info = ServerMessage::ServerInfo {
            message: format!("Hello{}World", esc),
        };
        network_info.queue_server_message(malicious_info);

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session_info.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                username::handle(
                    lobby_state,
                    &mut session_info,
                    &mut ui_info,
                    &mut network_info,
                )
            } else {
                panic!("expected Lobby state");
            };
            session_info.state = temp_state;
            result
        };

        assert_eq!(
            ui_info.messages.len(),
            1,
            "expected one server info message to be displayed"
        );
        assert_eq!(
            ui_info.messages[0],
            "Server: HelloWorld",
            "server info message was not correctly sanitized"
        );
    }
}
