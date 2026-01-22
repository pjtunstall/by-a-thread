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

use common;
use server;

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
        Show,                          // Restore the cursor, which is hidden during the
        MoveToColumn(0),               // countdown.
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

    let private_key = common::auth::private_key();
    let server_binding_addr = server::net::BINDING_ADDRESS;

    let socket = match common::net::bind_socket(server_binding_addr) {
        Ok(socket) => {
            println!("Server listening on {}.", server_binding_addr);
            socket
        }
        Err(e) => {
            eprintln!("error: failed to bind socket");
            eprintln!("details: {}", e);
            if e.kind() == io::ErrorKind::AddrInUse {
                eprintln!("is another instance of the server already running");
            }
            process::exit(1);
        }
    };

    server::run::run_server(socket, server_binding_addr, private_key);
}
