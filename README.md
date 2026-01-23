# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [How to play](#how-to-play)
  - [Controls](#controls)
  - [Objective](#objective)
- [Setup](#setup)
  - [To run locally](#to-run-locally)
  - [To run on docker](#to-run-on-docker)
  - [To run on Hetzner](#hetzner)
- [Levels](#levels)
- [Curiosities](#curiosities)
  - [Network debugging](#network-debugging)
  - [Docker: the dummy client trick](#docker-the-dummy-client-trick)
- [Credits](#credits)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

The game is not yet online, so real matches aren't possible. For now, you can at least get a sense of it by running the server and one or more clients on a single machine. My plan is to play test it first with friends, then host it publicly according the the plan outlined in [Security](docs/security.md).

## How to play

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.
- Left shift for sniper mode.

### Objective

Be the last one standing.

## Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`.

## To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `TARGET_HOST=127.0.0.1 cargo run --release --bin server` in one terminal. For each player, open another terminal and run `cargo run --release --bin client`. Then follow the prompts. The passcode will appear in the server terminal.

## To run locally on Docker

Assuming you've installed [Docker](https://www.docker.com/)--and started it with `sudo systemctl start docker` if need be--you can run the server on Docker and the client directly on your host machine. Build the server image:

```sh
# Extract the version number to tag the image with.
VERSION=$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2)

# Tag the image with the version number and as "latest".
docker build \
  -t server-image:$VERSION \
  -t server-image:latest \
  .
```

Then run the server:

```sh
docker run -d \
  --name server-container \
  --rm \
  -e TARGET_HOST=127.0.0.1 \
  -p 5000:5000/udp \
  server-image
```

Tell Docker to log output so far, so that we can the server banner with the passcode:

```sh
docker logs server-container
```

Then run the client as usual

(A container stops when its main process exits. In this case, the main process is the server. The server will exit shortly after the last client leaves. In case you want to stop it immediately, `stop server-container`.)

## To run on Hetzner

SSH into a Hetzner VPS. Transfer the latest image of the server:

```sh
docker save server-image | gzip | ssh hetzner 'gunzip | docker load'
```

Then, in SSH, run the server container:

```sh
# Set TARGET_HOST to the IP address of the VPS.
docker run -d \
 --name server-container \
 --rm \
 -e TARGET_HOST=$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) \
 -p 5000:5000/udp \
 server-image
```

And run the client locally, as usual with `cargo run --release --bin client`.

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

## Credits

### Sound effects

Sound effects from Yodguard and freesound_community via Pixabay.

### Images

The wall images for the first two levels are frescos from the Minoan palace at Knossos, Crete. The [griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) is adapted from a photo by Vadim Indeikin, and the [bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) from a photo by Gleb Simonov.

The skies for levels 2 and 3 ([blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091)) are by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
