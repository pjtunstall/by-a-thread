# Networking Debugging Lesson

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

### Network Address Concepts

### Binding Address vs Connectable Address

**Binding Address:**
- **Role**: Where a server listens for incoming connections.
- **Perspective**: Server-side only.
- **Purpose**: Binds to a specific network interface on the server machine.
- **Example**: `0.0.0.0:5000` means "listen on all interfaces at port 5000."

**Connectable Address:**
- **Role**: Where clients connect to reach the server.
- **Perspective**: Client-side (and shared knowledge).
- **Purpose**: The address clients use to establish connections.
- **Example**: `127.0.0.1:5000` means "connect to localhost at port 5000."

**Key Relationship:**
- The binding address determines *where the server listens*.
- The connectable address determines *where clients connect*.
- These can be different addresses, especially in more complex networked environments.

### Unspecified Address (`0.0.0.0`)

**For Binding Addresses:**
- `0.0.0.0` means "bind to all available network interfaces."
- The server will accept connections on any IP address the machine has.
- This is useful when you don't know which interface clients will use.
- It's like saying "I don't care what you call me."

**For Connectable Addresses:**
- `0.0.0.0` is invalid as a connectable address.

### Client Addresses

Clients also have binding addresses (where they bind locally), but:
- Clients typically bind to `0.0.0.0:0` (any interface, random port).
- The client's binding address is usually handled automatically by the OS.
- Clients only need to know the server's connectable address.

### How This Applies to the Current Project

**Server Side:**
- Uses `BINDING_ADDRESS` (`0.0.0.0:5000`) to listen on all interfaces.
- Uses `SERVER_CONNECTABLE_ADDRESS` (`127.0.0.1:5000`) in connect tokens.
- Renet's connect tokens contain the connectable address for clients.

**Client Side:**
- Uses `SERVER_CONNECTABLE_ADDRESS` as the default connection target.
- Client binding is handled automatically.

## Before Docker (Local Development)

**Server Setup:**
```rust
// server/src/net.rs
pub const BINDING_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5000);

// common/src/net.rs  
pub const SERVER_CONNECTABLE_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
```

**Client Setup:**
```rust
// client/src/lobby/state_handlers/server_address.rs
let default_server_connectable_addr = common::net::SERVER_CONNECTABLE_ADDRESS;
```

**How it worked:**
1. Server binds to `BINDING_ADDRESS` (`0.0.0.0:5000`) (all interfaces).
2. Client connects to `SERVER_CONNECTABLE_ADDRESS` (`127.0.0.1:5000`) (localhost).
3. **Server's public address in connect token: `BINDING_ADDRESS` (`0.0.0.0:5000`)**
4. Connection succeeded despite the address mismatch; I don't know why.

**Theories:**
- **Renet special handling**: Renet might have special logic for local connections that tolerates `0.0.0.0` in connect tokens when client and server are on the same machine.
- **OS-level routing**: The operating system might be translating `0.0.0.0` to `127.0.0.1` for local connections.
- **Network stack behavior**: The local network stack might be more permissive about address resolution for loopback connections.

## With Docker

**The Problem:**
1. Server container binds to `BINDING_ADDRESS` (`0.0.0.0:5000`) inside container.
2. Docker maps container port 5000 to host port 5000.
3. Client on host tries to connect to `SERVER_CONNECTABLE_ADDRESS` (`127.0.0.1:5000`).
4. **Using the binding address in connect tokens**, clients got `0.0.0.0:5000`.
5. **Connection failed** because, apparently in this environment, clients can't connect to `0.0.0.0`.

**The Fix:**
```rust
// server/src/net.rs
pub fn build_server_config(
    current_time: Duration,
    protocol_id: u64,
    server_binding_addr: SocketAddr,
    private_key: [u8; 32],
) -> ServerConfig {
    // Always use the connectable address in tokens, never the binding address.
    let public_server_addr = if server_binding_addr.ip().is_unspecified() {
        common::net::SERVER_CONNECTABLE_ADDRESS
    } else {
        server_binding_addr  // If binding to specific IP, use that as connectable.
    };

    ServerConfig {
        current_time,
        max_clients: MAX_PLAYERS,
        protocol_id,
        public_addresses: vec![public_server_addr],
        authentication: ServerAuthentication::Secure { private_key },
    }
}
```

**How it works now:**
1. Server binds to `BINDING_ADDRESS` (`0.0.0.0:5000`) inside container.
2. Docker maps container port 5000 to host port 5000.
3. Client on host connects to `SERVER_CONNECTABLE_ADDRESS` (`127.0.0.1:5000`).
4. Server's public address in connect token: `SERVER_CONNECTABLE_ADDRESS`.
5. **Connection succeeds** because client address matches token address exactly.

**In a Nutshell:**
- Binding addresses are where servers listen (`0.0.0.0` = all interfaces).
- Connectable addresses are where clients connect (must be specific).
- Connect tokens should contain connectable addresses, never binding addresses.
- Using `SERVER_CONNECTABLE_ADDRESS` in tokens works in both local and Docker environments.

**Note on Non-Unspecified Binding Addresses:**
If the server binds to a specific IP (e.g., `192.168.1.100:5000`), that same address typically becomes the connectable address. This is because when you bind to a specific interface, that's the address clients must use to reach you. However, this depends on network configuration and routing--in complex networks, even a specific binding address might not be the address clients use (e.g., behind NAT, load balancers, etc.).

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
