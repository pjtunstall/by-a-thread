# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [How to play](#how-to-play)
  - [Controls](#controls)
  - [Objective](#objective)
- [To run locally](#to-run-locally)
- [Levels](#levels)
- [FPS meter](#fps-meter)
- [Curiosities](#curiosities)
  - [Network debugging](#network-debugging)
  - [Docker: the dummy client trick](#docker-the-dummy-client-trick)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

The game is not yet publicly online. Proper matches will have to wait till then. My plan is to play test it first on a VPS, then make it public according the the plan outlined in [Security](docs/security.md). For now, you can get a taste of it by running server and client [locally](#to-run-locally) (on one machine). See also [Docker](#docs/docker.md) for an idea of how the server is being deployed for initial testing.

Looking ahead to distribution of the client binary, see the [Build](docs/build.md) guide.

## How to play

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.
- Left shift for sniper mode.
- Escape to quit/exit.

### Objective

- Single player: Escape in time.
- Multiplayer: Be the last one standing.

(Although essentially a multiplayer game, I decided to allow solo matches for convenience during development. I've kept the feature so that it's easier to take a casual look at the game. To make it more interesting, I added a challenge to escape the maze before the timer runs out.)

## Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`.

That said, with the map, the "harder" levels can be easier to navigate, as their algorithms tend to produce more direct paths between distant cells.

# FPS meter

As instructed, I've included an FPS (frames per second) meter to monitor in-game performance.

## To run locally

This section assumes you're on Linux. I haven't tried running the game locally on Windows or Mac. On Windows, at least, it may fail due to the technique I found to let the client discover its own binding address by pinging a known external address.

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --release -p server` (or `cargo run --release --bin server`) in one terminal. For each player, open another terminal and run `cargo run --release -p client` (or `cargo run --release --bin ByAThread`). Then follow the prompts.

As a shortcut, you can press Tab to connect to localhost. When connecting to a remote server, the client gets IP and PORT as environment variables from a `.env` file.

The passcode will appear in the server terminal.

## Curiosities

### Network debugging

[Network Debugging](docs/network-debugging.md) documents some lessons learnt while fixing a bug I had when I first tried to run the server on Docker.

### The dummy client trick

As I containerized the server using Docker, I came across a useful trick. The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.
