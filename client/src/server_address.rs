use std::net::SocketAddr;

const SERVER_PORT: u16 = 5000;

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
