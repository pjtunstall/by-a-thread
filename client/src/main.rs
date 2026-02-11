use macroquad::prelude::Conf;

use client::{
    self,
    lobby::ui::Gui,
    run::{self},
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
    let ui = Gui::new();
    let private_key = common::auth::private_key();

    run::run_client_loop(private_key, ui).await;
}
