# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [Status](#status)
- [Spec](#spec)
- [Extras](#extras)
- [How to play](#how-to-play)
  - [Objective](#objective)
  - [Controls](#controls)

## Overview

![demo_griffin](https://github.com/user-attachments/assets/8fed148d-2866-4326-b023-78205a68bcf6)

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a multiplayer first-person shooter from 1973.

I wrote my own collision and movement physics (drawing on what I learnt in an [earlier project on ray tracing](https://github.com/pjtunstall/a-ray-tracer-darkly)) and went to town with the networking.

I delegated window management, reading input, loading textures, rendering, and audio to Macroquad, a simple game framework. I used the Renet library for some networking abstractions over UDP.

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
- Statically linked binaries; images built in empty containers
- Docker socket proxy, restricting commands available to the matchmaker
- Open Policy Agent to prevent privilege escalation
- Secure session lifecycle:
  - Ephemeral tokens
  - Cleanup of game server containers
- Caddy reverse proxy, handling TLS termination and certificates

**Netcode:**

- Clock synchronization
- Reconciliation and prediction for local player
- Interpolation for remote players
- Extrapolation for bullets

For more information on specific topics, see the following documents:

- [Architecture](docs/architecture.md)
- [Netcode](docs/netcode.md)
- [Mazes](docs/mazes.md)
- [Security](docs/security.md)
- [Build](docs/build.md)
- [DevOps](docs/devops.md)
- [How to run the backend locally](docs/local-backend.md)

## How to play

![demo_octopus](https://github.com/user-attachments/assets/efa90aaa-28e7-4757-8478-fbea9d58f869)

Launch the client and choose default server.

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

## Status

Currently in private beta. Successfully tested on Hetzner VPS with the Linux AppImage, Linux deb, and Windows versions of the client.

Next steps:

- Test macOS (Intel and Apple Silicon)
- Test Linux rpm
- Continue incremental refactoring
- Review remaining TODOs in comments
- Handle feedback
- Publish on itch.io

Till then, please contact me if you'd like to play, and I'll send you a private link.

## Further developments

Possible further developments:

- Observability with Prometheus, Loki, and Grafana
- Blue-green deployment: Avoid the maintenance outage (for updates) by instead switching to a new VPS on a regular schedule. Provision this backup with the latest versions as the change-over time approaches. Then, when it's ready, start routing new-game requests to it. Wait till existing games have finished on the old VPS, then abandoning that one.
- Load testing
- Tests for matchmaker package
- Tests for client::matchmaker module with a mock HTTP server
- AI opponents
