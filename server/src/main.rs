use std::{
    env,
    io::{self, stdout},
    net::SocketAddr,
    process,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use crossterm::{
    cursor::{MoveToColumn, Show},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};

use common::{self, constants::SERVER_PORT};
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

    let private_key = {
        let s = env::var("PRIVATE_KEY").expect("could not find `PRIVATE_KEY` environment variable");
        let bytes = STANDARD.decode(s.trim()).unwrap_or_else(|e| {
            eprintln!("`PRIVATE_KEY` is not valid base64: {}", e);
            process::exit(1);
        });
        if bytes.len() != 32 {
            panic!(
                "`PRIVATE_KEY` must decode to exactly 32 bytes, got {}.",
                bytes.len()
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    };
    let public_host = env::var("IP").expect("could not find `IP` environment variable");
    let public_ip: std::net::IpAddr = public_host
        .parse()
        .expect("`IP` is not a valid IP address.");
    let public_port = env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(SERVER_PORT);
    let connectable_addr = SocketAddr::new(public_ip, public_port);

    let socket = match common::net::bind_socket(BINDING_ADDRESS) {
        Ok(socket) => {
            println!("Server listening on {}.", BINDING_ADDRESS);
            socket
        }
        Err(e) => {
            eprintln!("failed to bind socket: {}", e);
            if e.kind() == io::ErrorKind::AddrInUse {
                eprintln!("Is another instance of the server already running?");
            }
            process::exit(1);
        }
    };

    server::run::run_server(socket, connectable_addr, private_key);
}
