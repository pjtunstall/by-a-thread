use std::net::SocketAddr;

pub fn default_server_address() -> SocketAddr {
    // Puts the contents of the `.env` file into the string `embedded`, assuming
    // there's a `.env` is in the root directory. This will work on Windows too.
    // The compiler substitutes backslashes as needed. The contents of the
    // `.env` file should be two lines of the form:
    //
    //   IP=203.0.113.42
    // PORT=5000
    //
    // plus optional comments and whitespace. Substitute the IP and port number
    // of your default server.
    let embedded = include_str!("../../.env");
    let mut ip = "127.0.0.1";
    let mut port = "5000";
    for line in embedded.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                match key.trim() {
                    "IP" => ip = value,
                    "PORT" => port = value,
                    _ => {}
                }
            }
        }
    }
    format!("{}:{}", ip, port)
        .parse()
        .expect("invalid IP or PORT in embedded .env")
}
