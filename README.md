# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [How to play](#how-to-play)
  - [To run locally](#to-run-locally)
  - [Controls](#controls)
- [Levels](#levels)
- [Curiosities](#curiosities)
  - [Docker: dummy client trick](#docker-dummy-client-trick)
  - [Network debugging](#network-debugging)
- [Credits](#credits)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

## How to play

The game is not yet online, so real matches aren't possible. For now, you can, at least, get a sense of it by running the server and one or more clients on a single machine. My plan is to play test it first with friends, then host it publicly according the the plan outlined in [Security](docs/security.md).

### To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --release --bin server` in one terminal. For each client, open another terminal and run `cargo run --release --bin client`. Then follow the prompts. The passcode will appear in the server terminal.

### To run with the server on Docker

```sh
docker build -t server-image .
docker run -d \
  --name server-container \
  --rm \
  -p 5000:5000/udp \
  server-image
docker logs server-container # To see the server banner with the passcode.
```

You can then launch the client in the same terminal in the usual way: `cargo run --release --bin client`. When finished, stop the container:

```sh
docker stop server-container
```

If you wait a few seconds after the last client disconnected, the server will exit and then the container will stop of its own accord. The `--rm` flag with the `run` command ensures that the container will be deleted when it stops.

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.
- Left shift for sniper mode.

## Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`.

## Curiosities

### Docker: dummy client trick

As I containerized the server using Docker, I came across a useful trick. The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.

### Network debugging

In [Network Debugging Lesson](docs/network-debugging-lesson.md), I've included notes on a bug I had when I first tried to run the server on Docker. The bug was fixed, but the true cause has been lost among the various changes I made at the time. Still, I wanted to keep a record of what I learnt about networking and how to debug connectivity issues.

## Credits

### Sound effects

Sound effects from Yodguard and freesound_community via Pixabay.

### Images

The wall images for the first two levels are frescos from the Minoan palace at Knossos, Crete. The [griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) is adapted from a photo by Vadim Indeikin, and the [bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) from a photo by Gleb Simonov.

The [blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091) are by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
