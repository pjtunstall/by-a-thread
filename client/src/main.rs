use macroquad::prelude::Conf;

use client::{
    self,
    assets::Assets,
    lobby::ui::Gui,
    run::{self},
    session::ClientSession,
};
use common;

fn window_conf() -> Conf {
    Conf {
        window_title: "By a Thread".to_owned(),
        window_width: 1280,
        window_height: 720,
        // Although this is the default, the Makefile relies on it being explicitly set to
        // false. It toggles it to true when building the executable for
        // distribution and back to false afterwards.
        fullscreen: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let assets = Assets::load().await;
    let mut ui = Gui::new();

    let client_id = ::rand::random::<u64>();
    let mut session = ClientSession::new(client_id, None);

    let private_key = common::auth::private_key();

    let Some(choice) =
        run::prompt_for_server_address(&mut session, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    let (server_addr, connect_token, initial_lobby, share_passcode) = match choice {
        run::ConnectionChoice::Direct(addr) => (
            addr,
            None,
            client::state::Lobby::Passcode {
                prompt_printed: false,
            },
            None,
        ),
        run::ConnectionChoice::Matchmaker { api_host } => {
            let Some((addr, token, passcode, is_host)) =
                run::prompt_for_matchmaker_choice(&api_host, &mut ui, Some(&assets.font)).await
            else {
                return;
            };
            session.client_id = token.client_id;
            (
                addr,
                Some(token),
                client::state::Lobby::Connecting {
                    pending_passcode: Some(passcode.clone()),
                },
                is_host.then(|| passcode.string.clone()),
            )
        }
    };

    session.server_addr = Some(server_addr);
    session.transition(client::state::ClientState::Lobby(initial_lobby));

    run::run_client_loop(
        private_key,
        server_addr,
        connect_token,
        share_passcode,
        session,
        ui,
        assets,
    )
    .await;
}
