pub mod auth;
pub mod chat;
pub mod connecting;
pub mod countdown;
pub mod difficulty;
pub mod passcode;
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
        session::ClientSession,
        state::ClientState,
        test_helpers::{MockNetwork, MockUi},
    };
    use common::{
        auth::MAX_ATTEMPTS,
        input::sanitize,
        protocol::{AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE, ServerMessage},
    };

    #[test]
    fn client_banner_is_printed_correctly() {
        let mut ui = MockUi::default();
        let protocol_id = 12345;
        let server_addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let client_id = 99;

        let expected_banner =
            "Client Banner: Protocol=12345, Server=127.0.0.1:8080, ClientID=99".to_string();

        ui.print_client_banner(protocol_id, server_addr, client_id);

        assert_eq!(ui.messages, vec![expected_banner]);
        assert!(ui.errors.is_empty());
        assert!(ui.prompts.is_empty());
    }

    #[test]
    fn test_incoming_server_data_is_sanitized_before_display() {
        let bell = '\x07';
        let esc = '\x1B';

        let mut session_chat = ClientSession::new(0);
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

        chat::handle(&mut session_chat, &mut ui_chat, &mut network_chat, None);

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

        let mut session_auth = ClientSession::new(0);
        session_auth.transition(ClientState::Lobby(Lobby::Authenticating {
            waiting_for_input: false,
            waiting_for_server: false,
            guesses_left: MAX_ATTEMPTS,
        }));
        let mut ui_auth = MockUi::new();
        let mut network_auth = MockNetwork::new();

        let malicious_info = ServerMessage::ServerInfo {
            message: format!("{}{}", AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE, esc),
        };
        network_auth.queue_server_message(malicious_info);

        auth::handle(&mut session_auth, &mut ui_auth, &mut network_auth);

        assert_eq!(
            ui_auth.messages.len(),
            1,
            "expected one server info message to be displayed"
        );
        assert_eq!(
            ui_auth.messages[0],
            format!("Server: {}", AUTH_INCORRECT_PASSCODE_TRY_AGAIN_MESSAGE),
            "server info message was not correctly sanitized"
        );
        assert_eq!(ui_auth.prompts.len(), 1, "expected one prompt to be shown");
        assert_eq!(
            ui_auth.prompts[0],
            auth::passcode_prompt(MAX_ATTEMPTS - 1),
            "incorrect prompt shown after receiving server info"
        );
    }
}
