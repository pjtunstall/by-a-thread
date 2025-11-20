use std::{io, process};

use server;
use shared;

fn main() {
    ctrlc::set_handler(move || {
        println!("Server forced to shut down.");
        std::process::exit(0);
    })
    .expect("error setting Ctrl-C handler");

    let private_key = shared::auth::private_key();
    let server_addr = shared::net::server_address();

    let socket = match shared::net::bind_socket(server_addr) {
        Ok(socket) => {
            println!("Server listening on {}.", server_addr);
            socket
        }
        Err(e) => {
            eprintln!("Error: Failed to bind socket.");
            eprintln!("Details: {}.", e);
            if e.kind() == io::ErrorKind::AddrInUse {
                eprintln!("Is another instance of the server already running?");
            }
            process::exit(1);
        }
    };

    server::run::run_server(socket, server_addr, private_key);
}
