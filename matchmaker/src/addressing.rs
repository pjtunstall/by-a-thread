use std::net::{SocketAddr, ToSocketAddrs};

pub fn resolve_server_host(host: &str) -> std::net::IpAddr {
    let addrs: Vec<SocketAddr> = (host, 0u16)
        .to_socket_addrs()
        .expect("failed to resolve server host")
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .map(|a| a.ip())
        .or_else(|| addrs.first().map(|a| a.ip()))
        .expect("server host resolved to no addresses")
}
