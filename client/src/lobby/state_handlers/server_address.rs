use crate::{
    lobby::ui::LobbyUi,
    server_address,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::constants::SERVER_PORT;

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> Option<ClientState> {
    let Lobby::ServerAddress { prompt_printed } = lobby_state else {
        unreachable!();
    };

    if let Ok(Some(common::input::UiKey::Tab)) = ui.poll_single_key() {
        session.input_queue.clear();
        session.server_addr = Some(server_address::localhost_address());
        return Some(ClientState::Lobby(Lobby::Passcode {
            prompt_printed: false,
        }));
    }

    if let Some(input_string) = session.take_input() {
        match server_address::parse_server_address(&input_string, SERVER_PORT) {
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
        "Press Enter for default server (recommended),\nTab for localhost,\nor pick another server (host:port): ",
    )
}
