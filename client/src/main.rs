use macroquad::prelude::Conf;

use client::{
    self,
    lobby::ui::Gui,
    run::{self, WINDOW_HEIGHT, WINDOW_WIDTH},
};
use common;

fn window_conf() -> Conf {
    Conf {
        window_title: "By a Thread".to_owned(),
        window_width: WINDOW_WIDTH as i32,
        window_height: WINDOW_HEIGHT as i32,
        fullscreen: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let private_key = common::auth::private_key();

    let ui = Gui::new();

    run::run_client_loop(private_key, ui).await;
}
