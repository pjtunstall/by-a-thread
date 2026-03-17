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
- [Further developments](#further-developments)

## Overview

![demo_griffin](https://github.com/user-attachments/assets/8fed148d-2866-4326-b023-78205a68bcf6)

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a multiplayer first-person shooter from 1973.

My game features custom physics and latency compensation. I delagated input, rendering, and audio to [Macroquad](https://macroquad.rs/). [Renet](https://docs.rs/crate/renet/latest) provides the UDP-based networking layer.

## Status

Currently in private beta. Successfully tested on Hetzner VPS with the Linux AppImage, Linux deb, and Windows versions of the client. Waiting for feedback on macOS and RMP before making public. Till then, please contact me if you'd like to play, and I'll send you a personal link.

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

First set up the client. Let me know if you'd like a personal download link. Alternatively, to build from source, clone this repo and run the appropriate Make command for your system (`make windows`, `make macos-intel`, `make macos-silicon`, `make deb`, `make rpm`, `make appimage` from the project root). In either case, see the [installation guide](docs/installation.md) for OS-specific instructions.

Once installed, launch the client and choose "default server".

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

- Await confirmation that the yet-to-be-tested binaries work:
  - macOS (Intel and Apple Silicon)
  - Linux RPM
- Continue incremental refactoring
- Review remaining TODOs in comments
- Handle feedback
- Publish on itch.io

## Further developments

Possible further developments:

- A landing page
- Observability with Prometheus, Loki, and Grafana
- Blue-green deployment: Avoid the maintenance outage (for updates) by instead switching to a new VPS on a regular schedule. Provision this backup with the latest versions as the change-over time approaches. Then, when it's ready, start routing new-game requests to it. Wait till existing games have finished on the old VPS, then abandoning that one.
- Load testing
- Tests for matchmaker package
- Tests for client::matchmaker module with a mock HTTP server
- Fuzz tests for in-game logic.
- AI opponents
