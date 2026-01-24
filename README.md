# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [How to play](#how-to-play)
  - [Controls](#controls)
  - [Objective](#objective)
- [To run locally](#to-run-locally)
- [Levels](#levels)
- [Curiosities](#curiosities)
  - [Network debugging](#network-debugging)
  - [Docker: the dummy client trick](#docker-the-dummy-client-trick)
- [Credits](#credits)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

The game is not yet publicly online. Proper matches will have to wait till then. My plan is to play test it first with friends on a Hetzner VPS, then make it public according the the plan outlined in [Security](docs/security.md). For now, you can get a taste of it by running server and client [locally](#to-run-locally) (on one machine). See also [Docker](#docs/docker.md) for an idea of how the server is being deployed for initial testing.

Looking ahead to distribution of the client binary, see my [Installation](docs/installation.md) guide.

## How to play

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.
- Left shift for sniper mode.

### Objective

Be the last one standing.

## Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`. Ironically, the "harder" levels can be easier to navigate, in a way, especially with the help of a map, as their algorithms tend to produce more direct paths between distant cells. You can often see to the end of a deadend and so discount it. But how will maze style affect actual gameplay?

## To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --release --bin server` in one terminal. For each player, open another terminal and run `cargo run --release -p client` (or `cargo run --release --bin ByAThread`). Then follow the prompts. The passcode will appear in the server terminal.

(In production, the client will get IP and PORT as environment variables from a `.env` file.)

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

The [labyrinth icon](https://www.flaticon.com/free-icons/labyrinth) for the client executable is by Freepik via Flaticon.

The wall images for the first two levels are frescos from the Minoan palace at Knossos, Crete. The [griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) is adapted from a photo by Vadim Indeikin, and the [bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) from a photo by Gleb Simonov.

The skies for levels 2 and 3 ([blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091)) are by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
