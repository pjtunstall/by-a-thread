use client::{
    self,
    ui::{ClientUi, TerminalUi},
};
use shared;
use std::net::UdpSocket;

fn main() {
    let mut ui = match TerminalUi::new() {
        Ok(ui) => ui,
        Err(e) => {
            eprintln!("Failed to initialize terminal UI: {}.", e);
            eprintln!("Your terminal may be in an uninitialized state.");
            eprintln!("Try typing 'reset' and pressing Enter to fix it.");
            return;
        }
    };

    let private_key = shared::auth::private_key();
    let server_addr = shared::net::server_address();

    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(e) => {
            ui.show_error(&format!("Failed to bind client socket: {}.", e));
            return;
        }
    };

    client::run::run_client(socket, server_addr, private_key, &mut ui);
}
