use std::net::{IpAddr, SocketAddr};

use common::constants::SERVER_PORT;

#[cfg(not(test))]
use crate::config;

pub fn default_server_address() -> Result<SocketAddr, String> {
    #[cfg(not(test))]
    {
        use std::net::ToSocketAddrs;

        let host = config::game_server_host();
        let mut addrs = (host.as_str(), SERVER_PORT)
            .to_socket_addrs()
            .map_err(|e| format!("failed to resolve {}: {}", host, e))?;
        addrs
            .next()
            .ok_or_else(|| format!("no addresses for {}", host))
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
        "Invalid address. Press Enter, or Tab, or enter a domain (like example.com) or an IP address (like 192.168.0.10).",
    ))
}
