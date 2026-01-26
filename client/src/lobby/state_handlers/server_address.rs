use std::net::{IpAddr, SocketAddr};

use crate::{
    lobby::ui::LobbyUi,
    session::ClientSession,
    state::{ClientState, Lobby},
};

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> Option<ClientState> {
    let Lobby::ServerAddress { prompt_printed } = lobby_state else {
        unreachable!();
    };

    let default_server_connectable_addr = common::net::get_connectable_address();

    if let Ok(Some(common::input::UiKey::Tab)) = ui.poll_single_key() {
        let localhost_addr = SocketAddr::new(
            "127.0.0.1".parse().expect("failed to parse localhost"),
            default_server_connectable_addr.port(),
        );
        session.input_queue.clear();
        session.server_addr = Some(localhost_addr);
        return Some(ClientState::Lobby(Lobby::Passcode {
            prompt_printed: false,
        }));
    }

    if let Some(input_string) = session.take_input() {
        match parse_server_address(&input_string, default_server_connectable_addr) {
            Ok(parsed_server_addr) => {
                session.input_queue.clear();
                session.server_addr = Some(parsed_server_addr);
                return Some(ClientState::Lobby(Lobby::Passcode {
                    prompt_printed: false,
                }));
            }
            Err(message) => {
                ui.show_error(&message);
                ui.show_prompt(&server_address_prompt());

                *prompt_printed = true;
                return None;
            }
        }
    }

    if !*prompt_printed {
        ui.show_prompt(&server_address_prompt());
        *prompt_printed = true;
        return None;
    }

    None
}

fn server_address_prompt() -> String {
    format!(
        "Press Enter to connect to the default server,\n  Tab for localhost,\n  or choose another server (ip[:port]): ",
    )
}

fn parse_server_address(
    input: &str,
    default_server_connectable_addr: SocketAddr,
) -> Result<SocketAddr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_server_connectable_addr);
    }

    if let Ok(addr) = trimmed.parse::<SocketAddr>() {
        return Ok(addr);
    }

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_server_connectable_addr.port()));
    }

    Err(format!(
        "Invalid address. Press Enter, or Tab, or choose an IP like 192.168.0.10:5000.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockUi;

    #[test]
    fn returns_default_address_on_blank_input() {
        let default_server_connectable_addr = common::net::get_connectable_address();
        let parsed = parse_server_address("   ", default_server_connectable_addr)
            .expect("expected default address");
        assert_eq!(parsed, default_server_connectable_addr);
    }

    #[test]
    fn parses_ip_with_default_port() {
        let default_server_connectable_addr = common::net::get_connectable_address();
        let parsed = parse_server_address("192.168.1.50", default_server_connectable_addr)
            .expect("expected address");
        assert_eq!(
            parsed,
            SocketAddr::new(
                IpAddr::from([192, 168, 1, 50]),
                default_server_connectable_addr.port()
            )
        );
    }

    #[test]
    fn parses_ip_with_port() {
        let default_server_connectable_addr = common::net::get_connectable_address();
        let parsed = parse_server_address("192.168.1.50:6000", default_server_connectable_addr)
            .expect("expected address");
        assert_eq!(
            parsed,
            SocketAddr::new(IpAddr::from([192, 168, 1, 50]), 6000)
        );
    }

    #[test]
    fn invalid_input_reprompts() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::ServerAddress {
            prompt_printed: false,
        }));
        session.add_input("not-an-ip".to_string());
        let mut ui = MockUi::default();

        let next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(next_state.is_none());
        assert_eq!(ui.errors.len(), 1);
        assert_eq!(ui.prompts.len(), 1);
        match session.state {
            ClientState::Lobby(Lobby::ServerAddress { prompt_printed }) => {
                assert!(prompt_printed);
            }
            _ => panic!("expected ServerAddress state"),
        }
    }
}
