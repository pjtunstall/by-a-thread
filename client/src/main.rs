use macroquad::prelude::Conf;

use client::{
    self,
    assets::Assets,
    lobby::ui::Gui,
    narthex,
    run::{self},
    session::ClientSession,
};

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

    let Some(api_host) =
        narthex::prompt_for_server_address(&mut session, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    let Some(response) =
        narthex::seek_matchmaker_response(&api_host, &mut ui, Some(&assets.font)).await
    else {
        return;
    };

    let only_player = response.only_player();
    let server_addr = response.server_addr();
    let share_passcode = response.share_passcode();
    let connect_token = response.connect_token();

    session.client_id = connect_token.client_id;
    session.server_addr = Some(server_addr);
    session.transition(client::state::ClientState::Lobby(
        client::state::Lobby::Connecting {
            pending_passcode: Some(()),
        },
    ));

    run::run_client_loop(
        server_addr,
        connect_token,
        share_passcode,
        session,
        ui,
        assets,
        only_player,
    )
    .await;
}
