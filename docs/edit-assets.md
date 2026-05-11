# How to edit the assets

- [Linux](#linux)
- [Windows](#windows)
- [macOS](#macos)

This document explains how to edit the assets (fonts, sound effects, and images) of the client. For any platform, you can always perform a full build and installation, as described in `docs/build.md` and `docs/installation.md`. The following sections describe platform-specific shortcuts.

## Linux

On Linux, when loading assets (fonts, sound effects, and images), the client checks locations in this order:

1. `APPDIR/assets/...` (if `APPDIR` is set),
2. `/usr/lib/by-a-thread/...` (if that installed path exists),
3. `client/assets/...` in the repo (fallback).

Therefore, if you have a system-installed copy in `/usr/lib/by-a-thread`, running `cargo run --release -p client` from the repo can still use the installed assets instead of local files you just edited.

A quick alternative to performing a full build and installation is to set `APPDIR` as follows to ensure the new assets are used rather than the installed ones:

```sh
APPDIR="$PWD/client" cargo run --release -p client
```

or, to run in windowed mode,

```sh
APPDIR="$PWD/client" cargo run --release -p client -- --windowed
```

## Windows

On Windows, assets are embedded into the client binary at compile time.

That means editing files under `client/assets/` does not update an already-built executable. Rebuild and rerun the client so the new asset bytes are included:

```sh
cargo run --release -p client
```

If the new asset still does not appear, do a clean rebuild:

```sh
cargo clean -p client
cargo run --release -p client
```

To run in windowed mode, append `-- --windowed` to the run command.

## macOS

On macOS, assets are loaded from files at runtime, and the client checks locations in this order:

1. app bundle resources (when running from a `.app`),
2. `/usr/lib/by-a-thread/...` (if that installed path exists),
3. `client/assets/...` in the repo (fallback).

To run the client from the repo, enter `cargo run --release -p client` (or, for windowed mode, `cargo run --release -p client`). Local edits in `client/assets/` should be picked up immediately.
