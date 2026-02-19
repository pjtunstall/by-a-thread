use macroquad::prelude::Conf;

use client::{
    assets::Assets,
    lobby::ui::Gui,
    pre_lobby,
    run,
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
    let ui = Gui::new();

    let client_id = ::rand::random::<u64>();
    let session = ClientSession::new(client_id, None);

    let Some(result) = pre_lobby::run_pre_lobby_loop(session, ui, assets).await else {
        return;
    };

    run::run_client_loop(
        result.server_addr,
        result.connect_token,
        result.share_passcode,
        result.session,
        result.ui,
        result.assets,
        result.only_player,
    )
    .await;
}
