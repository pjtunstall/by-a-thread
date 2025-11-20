use std::net::UdpSocket;

use client::{self, run, ui::MacroquadUi};
use shared;

#[macroquad::main("By a Thread")]
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

    let ui = MacroquadUi::new();

    run::run_client_loop(socket, server_addr, private_key, ui).await;
}
