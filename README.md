# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [Status](#status)
- [Spec](#spec)
- [Extras](#extras)
- [How to play](#how-to-play)
  - [Setup](#setup)
  - [Multiplayer](#multiplayer)
  - [Single player](#single-player)
  - [Controls](#controls)
- [Next steps](#next-steps)
- [Possible further developments](#possible-further-developments)

## Overview

![demo_griffin](https://github.com/user-attachments/assets/8fed148d-2866-4326-b023-78205a68bcf6)

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a multiplayer first-person shooter from 1973.

My game features custom physics and latency compensation. I used [Macroquad](https://macroquad.rs/) for input, rendering, and audio, and [Renet](https://docs.rs/crate/renet/latest) for the UDP-based networking layer.

## Status

Successfully tested on Hetzner VPS with the Linux AppImage, Linux deb, and Windows versions of the client. Have you tried the macOS and RMP versions? Let me know if they worked, e.g. through the contact form [here](https://by-a-thread.de/).

## Spec

According to the 01 spec, the game should include:

- all elements of the original game:
  - multiplayer,
  - 3D, 1st person perspective,
  - shooting,
  - set in a maze;
- client-server architecture;
- communication via the UDP networking protocol;
- frames-per-second meter to monitor in-game performance;
- three levels with mazes of increasing difficulty, defined as more dead ends;
- option to connect to an arbitrary server.

## Extras

I went beyond the spec in a few ways.

**DevOps:**

- Cloud-hosted backend
- Matchmaker API that spawns game servers in response to client requests, allowing concurrent sessions
- Containerization with Docker Compose
- Makefile and associated scripts for build and deployment
- GitHub Actions:
  - to build whole project and push backend to Docker Hub
  - to deploy frontend to itch.io
- Cron job for scheduled deployment and updates
- Uptime monitoring with UptimeRobot

**Security:**

- Authentication and rate limiting
- Containerized components run with minimum privileges
- Statically linked binaries; images run in empty containers
- Docker socket proxy, restricting commands available to the matchmaker
- Open Policy Agent to guard against privilege escalation
- Secure session lifecycle:
  - Ephemeral tokens
  - Cleanup of game server containers
- Caddy reverse proxy, handling TLS termination and certificates

**Netcode:**

- Clock synchronization
- Reconciliation and prediction for local player
- Interpolation for remote players
- Extrapolation for bullets

More information on specific topics can be found in the docs:

- [Architecture](docs/architecture.md)
- [Netcode](docs/netcode.md)
- [Mazes](docs/mazes.md)
- [Security](docs/security.md)
- [Build](docs/build.md)
- [DevOps](docs/devops.md)
- [How to run the backend locally](docs/local-backend.md)

## How to play

![demo_octopus](https://github.com/user-attachments/assets/efa90aaa-28e7-4757-8478-fbea9d58f869)

### Setup

- Step 1: [Download](https://by-a-thread.de/) the client.
- Step 2: Install: See the [installation guide](docs/installation.md) for OS-specific instructions.
- Step 3: Play: Launch the game and choose "default server".

As an alternative to Step 1, if you prefer to build from source, clone this repo and run the appropriate Make command for your system from the project root (`make windows`, `make macos-intel`, `make macos-silicon`, `make deb`, `make rpm`, `make appimage`).

### Multiplayer

- One player chooses "New game".
- They'll see an access code to share.
- Other players choose "Join game" and enter this code.
- The first player to enter their name and join the the chat gets to decide the difficulty level and when to start the game.

Objective: Be the last one standing.

### Single player

- As above, but always choose "New game".

Objective: Escape before the timer runs out.

### Controls

In-game:

- W, A, S, D keys to move
- Arrow keys to turn
- SPACE to fire
- LEFT SHIFT for sniper mode

In-game or in the lobby:

- ESCAPE to quit/exit

## Next steps

- Get confirmation that the yet-to-be-tested binaries work:
  - macOS (Intel and Apple Silicon)
  - Linux RPM
- Handle any urgent issues raised by feedback
- Publish on itch.io

## Possible further developments

- A network connectivity indicator
- Observability:
  - Logs
  - Metrics
- Blue-green deployment: Avoid the maintenance outage (for updates) by instead switching to a new VPS on a regular schedule. Provision this backup with the latest versions as the change-over time approaches. Then, when it's ready, start routing new-game requests to it. Wait till existing games have finished on the old VPS, then abandoning that one.
- Load testing
- Benchmarking
- Tests for `client::matchmaker` module with a mock HTTP server
- Fuzz tests for in-game logic
- AI opponents; offline mode
- Sky images without hard seam
- Further incremental refactoring
- Review remaining TODOs in comments
