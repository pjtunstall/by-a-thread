use macroquad::prelude::*;

use client::{
    assets::Assets,
    exit,
    lobby::ui::{Gui, LobbyUi},
    pre_lobby,
    run,
    session::ClientSession,
};

fn window_conf() -> Conf {
    let force_windowed = std::env::args().any(|a| a == "--windowed")
        || std::env::var("BY_A_THREAD_WINDOWED")
            .map(|v| {
                let v = v.to_lowercase();
                v == "1" || v == "true" || v == "yes"
            })
            .unwrap_or(false);
    let default_fullscreen = !cfg!(debug_assertions);
    let fullscreen = !force_windowed && default_fullscreen;
    Conf {
        window_title: "By a Thread".to_owned(),
        window_width: 1280,
        window_height: 720,
        // Users can override with --windowed or BY_A_THREAD_WINDOWED=1 to work
        // around graphics driver issues (e.g. WGL_ARB_pixel_format on Windows).
        fullscreen,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut assets = Assets::load().await;
    let mut ui = Gui::new();

    let client_id = ::rand::random::<u64>();
    let mut session = ClientSession::new(client_id, None);

    loop {
        let Some(pre_lobby_result) =
            pre_lobby::run_pre_lobby_loop(session, ui, assets).await
        else {
            return;
        };

        let run_result = run::run_client_loop(
            pre_lobby_result.server_addr,
            pre_lobby_result.connect_token,
            pre_lobby_result.share_passcode,
            pre_lobby_result.session,
            pre_lobby_result.ui,
            pre_lobby_result.assets,
            pre_lobby_result.only_player,
        )
        .await;

        match run_result {
            run::RunClientReturn::Exit => return,
            run::RunClientReturn::ReturnToStartMenu {
                session: s,
                ui: u,
                assets: a,
            } => {
                session = s;
                ui = u;
                assets = a;
            }
            run::RunClientReturn::ConnectionError {
                message,
                session: s,
                ui: u,
                assets: a,
            } => {
                let mut connection_error_ui = u;
                connection_error_ui.show_sanitized_error(&message);
                connection_error_ui.show_message(" ");
                connection_error_ui.show_warning("Press ESCAPE to exit, ENTER to return to menu.");
                loop {
                    if is_key_pressed(KeyCode::Enter) {
                        session = s;
                        ui = connection_error_ui;
                        assets = a;
                        break;
                    }
                    if exit::should_quit() {
                        return;
                    }
                    connection_error_ui.draw(false, false, Some(&a.font), None);
                    next_frame().await;
                }
            }
        }
    }
}
