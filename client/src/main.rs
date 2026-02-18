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

    let Some(api_host) =
        run::prompt_for_server_address(&mut session, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    let Some((server_addr, connect_token, passcode, is_host)) =
        run::prompt_for_matchmaker_choice(&api_host, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    session.client_id = connect_token.client_id;
    session.server_addr = Some(server_addr);
    session.transition(client::state::ClientState::Lobby(
        client::state::Lobby::Connecting {
            pending_passcode: Some(()),
        },
    ));

    run::run_client_loop(
        private_key,
        server_addr,
        Some(connect_token),
        is_host.then(|| passcode.string.clone()),
        session,
        ui,
        assets,
    )
    .await;
}
