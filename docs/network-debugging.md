# Network Debugging

## Table of contents

- [Overview](#overview)
- [Network Address Concepts](#network-address-concepts)
  - [Binding Address vs Connectable Address](#binding-address-vs-connectable-address)
  - [Unspecified Address (`0.0.0.0`)](#unspecified-address-0000)
  - [Client Addresses](#client-addresses)
  - [How This Applies to the Current Project](#how-this-applies-to-the-current-project)
- [Current Network Setup](#current-network-setup)
  - [How It Works Locally](#how-it-works-locally)
  - [How It Works with Docker](#how-it-works-with-docker)
- [Debugging Tools and Commands](#debugging-tools-and-commands)
  - [Docker Commands](#docker-commands)
  - [Network Testing Commands](#network-testing-commands)
  - [Application-Level Debugging](#application-level-debugging)
  - [Common Issues and Solutions](#common-issues-and-solutions)
  - [Docker Port Mapping](#docker-port-mapping)
- [Network Architecture Summary](#network-architecture-summary)

## Overview

This document exists because of a networking bug that occurred when moving the server to Docker. The exact details of what caused the bug are not completely clear; see [How it works locally](#how-it-works-locally). Still, the debugging process taught me a thing or two. This document is a record of what I learnt about IP addresses, Docker port mapping and debugging tools.

## IP addresses and ports in By a Thread

### Network address concepts

### Binding address vs connectable address

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

- The binding address determines _where the server listens_.
- The connectable address determines _where clients connect_.
- These can be different addresses, especially in more complex networked environments.

### Unspecified address (`0.0.0.0`)

**For Binding Addresses:**

- `0.0.0.0` means "bind to all available network interfaces."
- The server will accept connections on any IP address the machine has.
- This is useful when you don't know which interface clients will use.
- It's like saying "I don't care what you call me."

**For Connectable Addresses:**

- `0.0.0.0` is invalid as a connectable address.

### Client addresses

Clients also have binding addresses (where they bind locally), but:

- Clients typically bind to `0.0.0.0:0` (any interface, random port).
- The client's binding address is usually handled automatically by the OS.
- Clients only need to know the server's connectable address.

### How this applies to the current project

**Server Side:**

- Uses `BINDING_ADDRESS` (`0.0.0.0:5000`) to listen on all interfaces.
- Uses `CONNECTABLE_ADDRESS` (`127.0.0.1:5000`) in connect tokens.
- Renet's connect tokens contain the connectable address for clients.

**Client Side:**

- Uses `CONNECTABLE_ADDRESS` as the default connection target.
- Client binding is handled automatically.

## Current network setup

**Server Configuration:**

```rust
// server/src/net.rs
pub const BINDING_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5000);

// common/src/net.rs
pub const CONNECTABLE_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
```

**Client Configuration:**

```rust
// client/src/lobby/state_handlers/server_address.rs
let default_server_connectable_addr = common::net::CONNECTABLE_ADDRESS;
```

### How it works locally

**Local Connection Flow:**

1. Server binds to `BINDING_ADDRESS` (`0.0.0.0:5000`), listening on all interfaces.
2. Client connects to `CONNECTABLE_ADDRESS` (`127.0.0.1:5000`), localhost.
3. Server's connect token contains `CONNECTABLE_ADDRESS` (`127.0.0.1:5000`).
4. Connection succeeds because client address matches token address exactly.

**Theory on the bug**

Item 3 is where I think the issue lay. In my original code, I was using the binding address (i.e. 0.0.0.0, i.e. unspecified) for the connect token, and, apparently, that had been working locally before I introduced Docker. At a later time, after it was working locally and on Docker the "correct" way (with the connectable address), I tried switching back to the binding address just to see if it would work locally--and it didn't. So, there's a bit of a mystery here. Probably I changed something else that affects it.

### How it works with Docker

**Docker Setup:**

```bash
docker build -t server-image .
docker run -d --name server-container --rm -p 5000:5000/udp server-image
```

**Docker Connection Flow:**

1. Server container binds to `BINDING_ADDRESS` (`0.0.0.0:5000`) inside container.
2. Docker maps container port 5000 to host port 5000.
3. Client on host connects to `CONNECTABLE_ADDRESS` (`127.0.0.1:5000`).
4. Server's connect token contains `CONNECTABLE_ADDRESS` (`127.0.0.1:5000`).
5. Connection succeeds because Docker forwards the connection correctly.

**Key Principle:** The same code works in both environments because:

- Binding address (`0.0.0.0:5000`) determines where the server listens.
- Connectable address (`127.0.0.1:5000`) determines where clients connect.
- Connect tokens always contain the connectable address, never the binding address.
- Docker's port mapping handles the translation between host and container networking.

**Note on Non-Unspecified Binding Addresses:** If the server binds to a specific IP (e.g., `192.168.1.100:5000`), that same address typically becomes the connectable address. This is because when you bind to a specific interface, that's the address clients must use to reach you. However, this depends on network configuration and routing--in complex networks, even a specific binding address might not be the address clients use (e.g., behind NAT, load balancers, etc.).

## Debugging tools and commands

### Docker commands

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

### Network testing commands

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

### Application-level debugging

**Check server logs for connection attempts:**

- Look for "Client X connected" messages
- Check for any error messages during connection attempts

**Check client error messages:**

- Look for disconnect reasons in the client output
- Common errors: "Connection timed out", "Connection denied"

### Common issues and solutions

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

### Docker port mapping

**Basic port mapping:**

```bash
docker run -p 5000:5000/udp server-image
```

**Format:** `-p <host_port>:<container_port>/<protocol>`

**Examples:**

- `-p 5000:5000/udp` - Map host UDP port 5000 to container UDP port 5000
- `-p 8080:5000/udp` - Map host UDP port 8080 to container UDP port 5000
- `-p 5000:5000/tcp` - Map host TCP port 5000 to container TCP port 5000

### Network architecture summary

```
Host Machine                    Docker Container
-----------                    ---------------
127.0.0.1:5000   <--maps-->    0.0.0.0:5000
     ^                           ^
     |                           |
Client connects           Server binds to all
to localhost              interfaces inside
                          container
```

The key insight is that Docker creates a network bridge between the host and container, and the port mapping makes the container's service appear as if it's running on the host machine.
