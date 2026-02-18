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

I wrote my own collision and movement physics (building on what I learnt in an [earlier project on ray tracing](https://github.com/pjtunstall/a-ray-tracer-darkly)) and went to town with the networking. For more details on what that entails and how I did it, see the [Netcode](docs/netcode.md) document.

I delegated window management, reading input, loading textures, rendering, and audio to Macroquad, a simple game framework. I used the Renet library for some networking abstractions over UDP.

For more on the structure of my project, see [Architecture](docs/architecture.md).

## Status

Successfully tested on VPS with the client on Ubuntu and on Windows; macOS (Intel and Apple Silicon) in the works. Currently designing a matchmaker to support concurrent sessions. See [Online plan](#online-plan).

## Spec

According to the 01 spec, the game is expected to include:

- all elements of the original game:
  - multiplayer,
  - 3D, 1st person perspective,
  - shooting,
  - set in a maze;
- client-server architecture;
- communication via the UDP networking protocol;
- frames-per-second meter to monitor in-game performance;
- three levels with mazes of increasing difficulty, defined as more dead ends; see below, [Levels](#levels);
- option to connect to an arbitrary server.

## How to play

![demo_octopus](https://github.com/user-attachments/assets/efa90aaa-28e7-4757-8478-fbea9d58f869)

### Objective

- Single player: Escape in time
- Multiplayer: Be the last one standing

### Controls

- WASD to move
- Arrow keys to turn
- Space to fire
- Left shift for sniper mode

- Escape to quit/exit

## Where to play

The game is not yet publicly online. Proper matches will have to wait till then...

### Locally

Clone this repo, `cd` into it. Install [Rust](https://rust-lang.org/tools/install/) and run `cargo run --release -p server` in one terminal. For each player, open another terminal and run `SERVER_ADDRESS=127.0.0.1:5000 cargo run --release -p client`. (Without `SERVER_ADDRESS`, the client tries to connect to by-a-thread.de; if it can't resolve or connect, it shows an error instead of crashing.) Follow the prompts to enter the passcode.

The passcode will appear in the server terminal.

### Online plan

Looking ahead to distribution of the client binary, see the [Build](docs/build.md) guide.

My plan is to play test it first on a VPS, then make it public according to the scheme outlined in [Security](docs/security.md). See [Docker](#docs/docker.md) for an idea of how the server is being deployed for initial testing.

## Levels

I've chosen to rank my mazes in terms of actual ease of navigation rather than tendency for dead ends (as per the spec), since the two are often at odds:

| **Level** | **Navigational ease** | **Dead-end density** |
| --- | --- | --- |
| 0 | Four-Quadrants Binary Tree | Standard Recursive Division (~11%) |
| 1 | Standard Recursive Division | Backtracker (~13%) |
| 2 | Meander[^1] | Territorial Recursive Division (~15%) |
| 3 | Territorial Recursive Division | Hecate's Key (~20%) |
| 4 | Hecate's Key | Wilson (~28%) |
| 5 | Prim | Kruskal (~38%) |
| 6 | Kruskal | Prim (~48%) |
| 7 | Drunkard's Walk | Drunkard's Walk (~50%) |
| 8 | Backtracker | Four-Quadrants Binary Tree (50% Fixed) |
| 9 | Wilson | Meander (50%+) |

My Territorial Recursive Division is Jamis Buck's [Better Recursive Division Algorithm](https://weblog.jamisbuck.org/2015/1/15/better-recursive-division-algorithm.html).

Percentages from Gemini, so take them with a pinch of salt. I haven't found a proof or experimental evidence for all of them yet. Gemini vacilates over whether recursive division or randomized backtracker has fewest dead ends, but the rankings don't shuffle wildly between responses. Its figures are roughly consistent with those that I have found, e.g. Mane et al. report DFS (i.e. Backtracker): 10.0, Wilson: 30.0, Kruskal: 30.6, Prim: 35.5.[^2] Their ranking of these algorithms in terms of difficulty also matches Gemini's.

## Links

### Maze-generating algorithms

- Jamis Buck: [The Buckblog](https://weblog.jamisbuck.org/archives.html).
- Jamis Buck: [Mazes for Programmers](http://www.mazesforprogrammers.com/).

### Netcode

- Gabriel Giambetta: [Fast-Paced Multiplayer](https://gabrielgambetta.com/client-server-game-architecture.html).

## Footnotes

[^1]: Some of these algorithms use different names internally: Meander is `TwiggyDividerQueue`, Hecate's Key is `BlobbyDividerQueue`, Drunkard's Walk is `TwiggyDividerRandom`, and Territorial Recursive Division is `BlobbyDividerRandom`.

[^2]: Deepak Mane, Rajat Harne, Tanmay Pol, Rashmi Asthagi, Sandip Shine, Bhushan Zope: [An Extensive Comparative Analysis on Different MazeGeneration Algorithms](https://ijisae.org/index.php/IJISAE/article/view/3557). International Journal of Intelligent Systems and Applications in Engineering. IJISAE, 2024, 12(2s), 37–47
