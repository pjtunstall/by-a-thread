use std::{
    env,
    io::{self, stdout},
    net::SocketAddr,
    process,
};

use crossterm::{
    cursor::{MoveToColumn, Show},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};

use common;
use server::{self, net::BINDING_ADDRESS};

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
    let public_host = env::var("TARGET_HOST")
        .expect("`TARGET_HOST` environment variable not set (e.g., `-e TARGET_HOST=127.0.0.1`)");
    let public_ip: std::net::IpAddr = public_host
        .parse()
        .expect("`TARGET_HOST` is not a valid IP address.");
    let connectable_addr = SocketAddr::new(public_ip, 5000);

    let socket = match common::net::bind_socket(BINDING_ADDRESS) {
        Ok(socket) => {
            println!("Server listening on {}.", BINDING_ADDRESS);
            socket
        }
        Err(e) => {
            eprintln!("error: failed to bind socket");
            eprintln!("details: {}", e);
            if e.kind() == io::ErrorKind::AddrInUse {
                eprintln!("Is another instance of the server already running?");
            }
            process::exit(1);
        }
    };

    server::run::run_server(socket, connectable_addr, private_key);
}
