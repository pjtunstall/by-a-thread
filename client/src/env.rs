use std::net::SocketAddr;

const SERVER_PORT: u16 = 5000;
#[cfg(not(test))]
const FALLBACK_SERVER_IP: &str = "46.225.7.153";

pub fn default_server_address() -> Result<SocketAddr, String> {
    #[cfg(test)]
    return Ok(SocketAddr::new(
        std::net::IpAddr::from([127, 0, 0, 1]),
        SERVER_PORT,
    ));

    #[cfg(not(test))]
    {
        if let Ok(addr) = std::env::var("SERVER_ADDRESS") {
            addr.parse()
                .map_err(|_| "SERVER_ADDRESS must be host:port (e.g. 127.0.0.1:5000)".to_string())
        } else {
            use std::net::ToSocketAddrs;
            ("by-a-thread.de", SERVER_PORT)
                .to_socket_addrs()
                .ok()
                .and_then(|mut addrs| addrs.next())
                .map(Ok)
                .unwrap_or_else(|| {
                    (FALLBACK_SERVER_IP, SERVER_PORT)
                        .to_socket_addrs()
                        .map_err(|e| format!("fallback address invalid: {}", e))?
                        .next()
                        .ok_or_else(|| "no addresses for fallback".to_string())
                })
        }
    }
}
