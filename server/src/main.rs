use std::{
    io::{self, stdout},
    process,
};

use crossterm::{
    cursor::{MoveToColumn, Show},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};

use server;
use shared;

struct Defer {}

impl Defer {
    fn new() -> Self {
        execute!(stdout(), Show).ok();
        Defer {}
    }
}

impl Drop for Defer {
    fn drop(&mut self) {
        execute!(stdout(), Show).ok();
    }
}

fn main() {
    let _defer = Defer::new();

    ctrlc::set_handler(move || {
        execute!(
            stdout(),
            Show,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine), // In particular, clear the "Game starting..." line.
            Print("\r\n")                  // Print a newline for the shell prompt.
        )
        .ok();
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
