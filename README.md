# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [To run locally](#to-run-locally)
- [How to play](#how-to-play)
  - [Controls](#controls)
  - [Objective](#objective)
- [Levels](#levels)
- [Network debugging](#network-debugging)
- [Credits](#credits)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

The game is not yet online, so real matches aren't possible. For now, you can at least get a sense of it by running the server and one or more clients on a single machine. My plan is to play test it first with friends, then host it publicly according the the plan outlined in [Security](docs/security.md).

## To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --release --bin server` in one terminal. For each player, open another terminal and run `cargo run --release --bin client`. Then follow the prompts. The passcode will appear in the server terminal.

Alternatively, see [Docker](docs/docker.md) for how to run the server on Docker.

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

## Network debugging

[Network Debugging](docs/network-debugging.md) documents some lessons learnt while fixing a bug I had when I first tried to run the server on Docker.

## Credits

### Sound effects

Sound effects from Yodguard and freesound_community via Pixabay.

### Images

The wall images for the first two levels are frescos from the Minoan palace at Knossos, Crete. The [griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) is adapted from a photo by Vadim Indeikin, and the [bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) from a photo by Gleb Simonov.

The [blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091) are by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
