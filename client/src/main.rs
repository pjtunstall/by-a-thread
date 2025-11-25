use macroquad::prelude::Conf;

use std::net::UdpSocket;

use client::{self, lobby::ui::MacroquadLobbyUi, run};
use shared;

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
    let server_addr = shared::net::server_address();
    let private_key = shared::auth::private_key();

    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind client socket: {}", e);
            return;
        }
    };

    let ui = MacroquadLobbyUi::new();

    run::run_client_loop(socket, server_addr, private_key, ui).await;
}
