use macroquad::prelude::Conf;

use client::{self, lobby::ui::Gui, run};
use common;

fn window_conf() -> Conf {
    Conf {
        window_title: "By a Thread".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let private_key = common::auth::private_key();

    let ui = Gui::new();

    run::run_client_loop(private_key, ui).await;
}
