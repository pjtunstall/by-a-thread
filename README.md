# By a Thread

![screenshot](screenshot.jpg)

- [Overview](#overview)
- [How to play](#how-to-play)
  - [To run locally](#to-run-locally)
  - [Controls](#controls)
- [Levels](#levels)
- [Workflow](#workflow)
- [Credits](#credits)

## Overview

This is my response to the 01Edu/01Founders challenge [multiplayer-fps](https://github.com/01-edu/public/tree/master/subjects/multiplayer-fps) (commit bb1e883). The aim is to remake [Maze](<https://en.wikipedia.org/wiki/Maze_(1973_video_game)>), a 3D multiplayer first-person shooter from 1973. We're asked to implement a client-server architecture and communicate via the UDP networking protocol.

I used Macroquad, a simple game framework, for window management, reading input, loading textures, rendering, and audio. I used the Renet library for some networking abstractions over UDP. On the other hand, I wrote the collision and movement physics, and went to town rolling my own netcode. For more details on that, see the [Netcode](docs/netcode.md) document. For more on the structure of my code, see [Architecture](docs/architecture.md).

## How to play

It's not yet hosted, so actually playing with other players is not yet practical, but you can at least run the server and one or more clients on a single machine.

### To run locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --bin server` in one terminal. For each client, open another terminal and run `cargo run --bin client`. Then follow the prompts. The passcode will appear in the server terminal.

### Controls

- WASD to move.
- Arrow keys to turn.
- Space to fire.

## Levels

As instructed, I've implemented three difficulty levels. The 01 instructions define difficulty as the tendency of a maze to have dead ends. I chose three maze-generating algorithms for this, in order of increasing difficulty: `Backtrack`, `Wilson`, and `Prim`. In practice, my impression so far is that the algorithms that lead to more deadends might actually be easier to navigate, especially with the map available, and the fact that you can generally see that a path is blocked before you commit to it.

## Workflow

I used a variety of AI chatbots to straighten out my understanding of the netcode and develop a strategy, to help troubleshoot, and to review my attempts. Gemini (Pro) has been my mainstay this time. It's really impressed me, especially since the launch of Gemini 3. Towards the end, I tried out two coding agents: Codex and Cascade. They've been a fantastic time-saver for tidying up the loose ends.

## Security

Currently, as a shortcut during development, the client simply imports (what should be) a private key and uses it to create the token needed to establish a Renet connection with the server. The server logs a random passcode to the terminal, different each game. This can be shared with any players who want to join the game. The first to join is designated the host, which just means they get to choose the difficulty level, triggering the start of the game itself.

In production, the private key should stay with a "matchmaker". The matchmaker, in this case, will just be a program responsible for processing requests to start a new game. (It won't attempt to match players with strangers for now.) If the number of games in progress is below the maximum, the matchmaker will launch a new instance of the game server and supply the host with a token, generated from a genuinely private key, along with a passcode and port number to join. The host can then share the passcode and port number with friends. (The host could perhaps be given a distinct passcode to ensure that the server recognizes them as the host; or it could go by order of arrival.) When the host chooses a difficulty level, the server will notify the matchmaker so that it can update its game count. Once the matchmaker has acknowledged that it received the message, the game proper can begin. The matchmaker will continuously sweep for expired games so that it can free up slots.

## Credits

### Sound effects

Sound effects from Yodguard and freesound_community via Pixabay.

### Images

The wall images for the first two levels are frescos from the Minoan palace at Knossos, Crete. The [griffin](https://en.wikipedia.org/wiki/Knossos#/media/File:%D0%A0%D0%BE%D1%81%D0%BF%D0%B8%D1%81%D1%8C*%D1%82%D1%80%D0%BE%D0%BD%D0%BD%D0%BE%D0%B3%D0%BE*%D0%B7%D0%B0%D0%BB%D0%B0.*%D0%9C%D0%B8%D0%BD%D0%BE%D0%B9%D1%81%D0%BA%D0%B8%D0%B9*%D0%B4%D0%B2%D0%BE%D1%80%D0%B5%D1%86._Knossos._Crete._Greece.*%D0%98%D1%8E%D0%BB%D1%8C*2013*-_panoramio.jpg) is adapted from a photo by Vadim Indeikin, and the [bull](https://commons.wikimedia.org/wiki/File:Knossos_bull_leaping_fresco.jpg) from a photo by Gleb Simonov.

The [blue rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=716572) and [white rust texture](https://www.publicdomainpictures.net/en/free-download.php?image=rust-grunge-background-texture&id=427091) are by Martina Stokow, via publicdomainpictures.net.

The player avatar image is derived from a computer model of the cosmos display of the Antikythera mechanism, from [Wikicommons](https://commons.wikimedia.org/wiki/File:41598_2021_84310_Fig7_HTML.jpg), and ultimately Freeth, T., Higgon, D., Dacanalis, A. et al.: [A Model of the Cosmos in the ancient Greek Antikythera Mechanism](https://www.nature.com/articles/s41598-021-84310-w).[^1]

[^1]: Freeth, T., Higgon, D., Dacanalis, A. et al. A Model of the Cosmos in the ancient Greek Antikythera Mechanism. Sci Rep 11, 5821 (2021). [https://doi.org/10.1038/s41598-021-84310-w](https://doi.org/10.1038/s41598-021-84310-w)
