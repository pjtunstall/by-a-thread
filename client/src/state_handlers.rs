pub mod auth;
pub mod chat;
pub mod connecting;
pub mod countdown;
pub mod difficulty;
pub mod game;
pub mod startup;
pub mod username;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        session::ClientSession,
        state::ClientState,
        test_helpers::{MockNetwork, MockUi},
        ui::ClientUi,
    };
    use shared::{auth::MAX_ATTEMPTS, protocol::ServerMessage};

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
        assert!(ui.status_lines.is_empty());
    }

    #[test]
    fn test_incoming_server_data_is_sanitized_before_display() {
        let bell = '\x07';
        let esc = '\x1B';

        let mut session_chat = ClientSession::new(0);
        session_chat.transition(ClientState::InChat);
        session_chat.mark_initial_roster_received();
        let mut ui_chat = MockUi::new();
        let mut network_chat = MockNetwork::new();

        let malicious_chat = ServerMessage::ChatMessage {
            username: format!("User{}", bell),
            content: format!("Hello{}World", esc),
        };
        network_chat.queue_server_message(malicious_chat);

        chat::handle(&mut session_chat, &mut ui_chat, &mut network_chat);

        assert_eq!(ui_chat.messages.len(), 1);
        assert_eq!(ui_chat.messages[0], "User: HelloWorld");

        let mut session_user = ClientSession::new(0);
        session_user.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
            awaiting_confirmation: true,
        });
        let mut ui_user = MockUi::new();
        let mut network_user = MockNetwork::new();

        let malicious_error = ServerMessage::UsernameError {
            message: format!("Name{}Taken", bell),
        };
        network_user.queue_server_message(malicious_error);

        username::handle(&mut session_user, &mut ui_user, &mut network_user);

        assert_eq!(ui_user.errors.len(), 1);
        assert_eq!(ui_user.errors[0], "Username error: NameTaken");
        assert_eq!(ui_user.messages.len(), 1);
        assert_eq!(ui_user.messages[0], "Please try a different username.");

        let mut session_auth = ClientSession::new(0);
        session_auth.transition(ClientState::Authenticating {
            waiting_for_input: false,
            guesses_left: MAX_ATTEMPTS,
        });
        let mut ui_auth = MockUi::new();
        let mut network_auth = MockNetwork::new();

        let malicious_info = ServerMessage::ServerInfo {
            message: format!("Incorrect passcode. Try again.{}", esc),
        };
        network_auth.queue_server_message(malicious_info);

        auth::handle(&mut session_auth, &mut ui_auth, &mut network_auth);

        assert_eq!(ui_auth.messages.len(), 1);
        assert_eq!(
            ui_auth.messages[0],
            "Server: Incorrect passcode. Try again."
        );
        assert_eq!(ui_auth.prompts.len(), 1);
        assert_eq!(ui_auth.prompts[0], auth::passcode_prompt(MAX_ATTEMPTS - 1));
    }
}
