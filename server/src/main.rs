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

pub struct Defer;

impl Defer {
    fn new() -> Self {
        Self {}
    }
}

impl Drop for Defer {
    fn drop(&mut self) {
        execute!(stdout(), Show).ok();
        clean_up();
    }
}

fn clean_up() {
    execute!(
        stdout(),
        Show,
        MoveToColumn(0),
        Clear(ClearType::CurrentLine), // In particular, clear the "Game starting..." line.
        Print("\r\n")                  // Print a newline for the shell prompt.
    )
    .ok();
}

fn main() {
    let _defer = Defer::new();

    ctrlc::set_handler(move || {
        clean_up();
        println!("Server forced to shut down.");
        std::process::exit(0);
    })
    .ok();

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
