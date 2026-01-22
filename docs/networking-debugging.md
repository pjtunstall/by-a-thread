# Networking and Debugging Guide

- [IP Addresses and Ports in By a Thread](#ip-addresses-and-ports-in-by-a-thread)
  - [Basic Concepts](#basic-concepts)
  - [Unspecified Address (`0.0.0.0`)](#unspecified-address-0000)
  - [Before Docker (Local Development)](#before-docker-local-development)
  - [With Docker](#with-docker)
- [Debugging Tools and Commands](#debugging-tools-and-commands)
  - [Docker Commands](#docker-commands)
  - [Network Testing Commands](#network-testing-commands)
  - [Application-Level Debugging](#application-level-debugging)
  - [Common Issues and Solutions](#common-issues-and-solutions)
  - [Docker Port Mapping](#docker-port-mapping)
- [Network Architecture Summary](#network-architecture-summary)

## IP Addresses and Ports in By a Thread

### Basic Concepts

- **IP Address**: Identifies a machine on a network
- **Port**: Identifies a specific service/application on that machine
- **Socket**: Combination of IP address + port (e.g., `127.0.0.1:5000`)

### Unspecified Address (`0.0.0.0`)

**For Servers:**
- `0.0.0.0` means "bind to all available network interfaces"
- Server will accept connections on any IP address the machine has
- Commonly used when you want the server accessible from multiple networks

**For Clients:**
- Clients typically bind to `0.0.0.0` (or `0.0.0.0:0` for random port) as their local address
- This means "connect from any available local interface"
- The actual connection target is the server's address

### Before Docker (Local Development)

**Server Setup:**
```rust
// common/src/net.rs
pub fn server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5000)
}
```

**Client Setup:**
```rust
// client/src/net.rs
pub fn default_server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}
```

**How it worked:**
1. Server binds to `0.0.0.0:5000` (all interfaces)
2. Client connects to `127.0.0.1:5000` (localhost)
3. Server's public address in connect token: `0.0.0.0:5000`
4. **Renet has special handling for local connections** that allows `0.0.0.0` in tokens when client and server are on the same machine
5. Connection succeeds despite the address mismatch

### With Docker

**The Problem:**
1. Server container binds to `0.0.0.0:5000` inside container
2. Docker maps container port 5000 to host port 5000
3. Client on host tries to connect to `127.0.0.1:5000`
4. Server's public address in connect token: `0.0.0.0:5000`
5. **Docker's port mapping breaks Renet's special handling** for `0.0.0.0` tokens
6. **Connection fails** because `127.0.0.1:5000` ≠ `0.0.0.0:5000` in literal address comparison

**The Issue:**
- `0.0.0.0` is a binding address, not a connectable address
- Connect tokens should contain the actual address clients will use
- Renet was permissive about this for local connections, but Docker's port mapping exposes the problem

**The Fix:**
```rust
// server/src/net.rs
pub fn build_server_config(
    current_time: Duration,
    protocol_id: u64,
    server_addr: SocketAddr,
    private_key: [u8; 32],
) -> ServerConfig {
    // When running in Docker, the server binds to 0.0.0.0 but clients connect to 127.0.0.1
    // Use 127.0.0.1 as the public address if the server is bound to 0.0.0.0.
    let public_addr = if server_addr.ip().is_unspecified() {
        SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            server_addr.port()
        )
    } else {
        server_addr
    };
    
    ServerConfig {
        current_time,
        max_clients: MAX_PLAYERS,
        protocol_id,
        public_addresses: vec![public_addr],
        authentication: ServerAuthentication::Secure { private_key },
    }
}
```

**How it works now:**
1. Server binds to `0.0.0.0:5000` inside container
2. Docker maps container port 5000 to host port 5000
3. Client on host connects to `127.0.0.1:5000`
4. Server's public address in connect token: `127.0.0.1:5000` (fixed!)
5. **Connection succeeds** because client address matches token address exactly

## Debugging Tools and Commands

### Docker Commands

**Check running containers:**
```bash
docker ps
```

**View container logs:**
```bash
docker logs server-container
docker logs --tail 20 server-container  # Last 20 lines
```

**Check port mappings:**
```bash
docker port server-container
```

**Execute commands inside container:**
```bash
docker exec server-container <command>
```

**Stop and remove containers:**
```bash
docker stop server-container
docker rm server-container
```

### Network Testing Commands

**netcat (nc) - Test UDP connectivity:**
```bash
# Test if port is accessible (timeout after 3 seconds)
timeout 3 nc -u 127.0.0.1 5000 </dev/null && echo "Port accessible" || echo "Port not accessible"

# Send test data
echo "test" | nc -u 127.0.0.1 5000
```

**ss - Socket statistics:**
```bash
# Check listening UDP ports
ss -ulnp | grep 5000

# Check all UDP ports
ss -ulnp
```

**netstat - Network statistics (if available):**
```bash
# Check listening ports
netstat -ulnp | grep 5000

# Check all listening ports
netstat -ulnp
```

### Application-Level Debugging

**Check server logs for connection attempts:**
- Look for "Client X connected" messages
- Check for any error messages during connection attempts

**Check client error messages:**
- Look for disconnect reasons in the client output
- Common errors: "Connection timed out", "Connection denied"

### Common Issues and Solutions

**Port not accessible:**
1. Check if Docker container is running: `docker ps`
2. Check port mapping: `docker port server-container`
3. Check if server is listening: `docker exec server-container ss -ulnp`

**Connection refused:**
1. Server might not be running inside container
2. Wrong port mapping in Docker run command
3. Firewall blocking the connection

**Authentication failures:**
1. Check if client and server have the same protocol version
2. Verify the passcode is correct
3. Check if connect token is valid (not expired)

### Docker Port Mapping

**Basic port mapping:**
```bash
docker run -p 5000:5000/udp server-image
```

**Format:** `-p <host_port>:<container_port>/<protocol>`

**Examples:**
- `-p 5000:5000/udp` - Map host UDP port 5000 to container UDP port 5000
- `-p 8080:5000/udp` - Map host UDP port 8080 to container UDP port 5000
- `-p 5000:5000/tcp` - Map host TCP port 5000 to container TCP port 5000

### Network Architecture Summary

```
Host Machine                    Docker Container
-----------                    ---------------
127.0.0.1:5000  <--maps-->    0.0.0.0:5000
     ^                           ^
     |                           |
Client connects           Server binds to all
to localhost              interfaces inside
                         container
```

The key insight is that Docker creates a network bridge between the host and container, and the port mapping makes the container's service appear as if it's running on the host machine.
