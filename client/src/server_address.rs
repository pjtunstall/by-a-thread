use std::net::{IpAddr, SocketAddr};

use common::constants::SERVER_PORT;

pub fn default_server_address() -> Result<SocketAddr, String> {
    #[cfg(not(test))]
    {
        use std::net::ToSocketAddrs;

        let mut addrs = ("api.by-a-thread.de", SERVER_PORT)
            .to_socket_addrs()
            .map_err(|e| format!("failed to resolve api.by-a-thread.de: {}", e))?;
        addrs
            .next()
            .ok_or_else(|| "no addresses for api.by-a-thread.de".to_string())
    }

    #[cfg(test)]
    return Ok(SocketAddr::new(
        std::net::IpAddr::from([127, 0, 0, 1]),
        SERVER_PORT,
    ));
}

pub fn localhost_address() -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), SERVER_PORT)
}

pub fn parse_server_address(input: &str, default_port: u16) -> Result<SocketAddr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return default_server_address();
    }

    if let Ok(addr) = trimmed.parse::<SocketAddr>() {
        return Ok(addr);
    }

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }

    Err(format!(
        "Invalid address. Press Enter, or Tab, or enter host:port like 192.168.0.10:5000.",
    ))
}
