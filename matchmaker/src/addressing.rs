use std::net::ToSocketAddrs;

pub fn resolve_server_host(host: &str) -> std::net::IpAddr {
    (host, 0u16)
        .to_socket_addrs()
        .expect("failed to resolve server host")
        .next()
        .expect("server host resolved to no addresses")
        .ip()
}
