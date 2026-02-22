# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [Spec](#spec)
- [How to play](#how-to-play)
  - [Objective](#objective)
  - [Controls](#controls)
- [Where to play](#where-to-play)
  - [Locally](#locally)
  - [Online plan](#online-plan)
- [Links](#links)
  - [Maze-generating algorithms](#maze-generating-algorithms)
  - [Netcode](#netcode)

## Overview

![demo_griffin](https://github.com/user-attachments/assets/8fed148d-2866-4326-b023-78205a68bcf6)

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a multiplayer first-person shooter from 1973.

I wrote my own collision and movement physics (drawing on what I learnt in an [earlier project on ray tracing](https://github.com/pjtunstall/a-ray-tracer-darkly)) and went to town with the networking.

I delegated window management, reading input, loading textures, rendering, and audio to Macroquad, a simple game framework. I used the Renet library for some networking abstractions over UDP.

I went beyond the spec in a few ways:

- cloud-hosted backend;
- matchmaker API that spawns game servers in response to client requests, allowing for concurrent sessions;
- containerization with Docker Compose;
- Makefile for build and deployment;

For more information on specific topics, see the following documents:

- [Architecture](docs/architecture.md)
- [Build](docs/build.md)
- [DevOps](docs/devops.md)
- [Netcode](docs/netcode.md)
- [Mazes](docs/mazes.md)
- [Security](docs/security.md)

## Status

Currently in private beta. Successfully tested on a VPS with the Linux AppImage, Linux deb, and Windows versions of the client.

Next steps include:

- CI/CD: GitHub Actions to build, and to deploy on a scheduled or emergency basis (backend to VPS and client to itch.io);
- test macOS (Intel and Apple Silicon);
- test Linux rpm;
- make public on itch.io;

Please contact me if you'd like to play.

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

## How to play

![demo_octopus](https://github.com/user-attachments/assets/efa90aaa-28e7-4757-8478-fbea9d58f869)

### Objective

- Single player: Escape in time
- Multiplayer: Be the last one standing

### Controls

- WASD to move
- Arrow keys to turn
- SPACE to fire
- LEFT SHIFT for sniper mode

- ESCAPE to quit/exit
