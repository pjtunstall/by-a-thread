# Installation

- [Overview](#overview)
- [Windows](#windows)
- [Linux]
  - [Building the .deb package](#building-the-deb-package)
  - [Installing the package](#installing-the-package)
  - [Package contents](#package-contents)

## Overview

This document describes how to create executable files or packages for various systems. It assumes you're creating them on Linux.

## Windows

Here is a guide to building and installing for Windows.

### Build files

Specific to the Windows build process are these components of the `client` directory:

- `src/build.rs` - Build script that compiles the icon resource
- `icon.rc` - Resource file specifying the icon to embed
- `icon.png` - The icon image file
- `Cargo.toml` sections:
  - `[build-dependencies]` with `embed-resource = "3.0.6"`
  - `[[bin]]` section defining the `ByAThread` binary

### Prerequisites

Install the Windows target and MinGW toolchain:

```sh
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64
```

### Building the executable

From the client directory:

```sh
cd client
cargo build --release --target x86_64-pc-windows-gnu
```

The executable will be created at `target/x86_64-pc-windows-gnu/release/ByAThread.exe` (relative to the workspace root).

### Distribution

The Windows executable is self-contained with the icon and all assets embedded. Simply distribute the `.exe` file. Users can run the executable directly without any installation process.

On first run, the game automatically extracts attribution and license files to `%APPDATA%\by-a-thread\` by calling `extract_license_files_to_user_directory()`:

- `LICENSE` - Game license
- `CREDITS.md` - Asset credits and licenses
- `FONT_LICENSE.txt` - NotoSerifBold font license (Apache 2.0)

This provides users with easy access to all required licenses while maintaining the convenience of a single executable distribution.

## Linux

This section describes how to build and install the Debian package for By a Thread on Linux systems. The package includes the game binary, assets, icon, and desktop file for easy installation.

Once installed, you should see an icon in your applications menu. Click it to launch the game. Note that, when you launch the game, it a plain icon with a cogwheel will appear in your taskbar, to represent the game instance, rather than a dot appearing beside the icon you clicked to launch it. As I understand it, this is because Macroquad, the library I used for window management, doesn't support full taskbar integration.

### Build files

The Linux Debian package build, in particular, involves these components of the `client` directory:

- `icon.png` - Icon file for the application
- `by-a-thread.desktop` - Desktop file for applications menu
- `Cargo.toml` sections:
  - `[package.metadata.deb]` with package metadata and asset paths
  - `[[bin]]` section defining the `ByAThread` binary

### Building the .deb package

To create the Debian package, you need to build from the client directory:

```sh
cd client
cargo build --release
cargo deb
```

The package will be created at `target/debian/by-a-thread_0.1.0-1_amd64.deb` (relative to the workspace root).

#### Why the `-1` suffix?

The `-1` in the filename is the Debian package revision number. It indicates this is the first revision of version 0.1.0. If you make changes to the package without changing the version number, you would increment this to `-2`, `-3`, etc.

### Installing the package

If you're still in `client` folder, move back to the the workspace root, then install the package using `dpkg`:

```sh
cd ..
sudo dpkg -i target/debian/by-a-thread_0.1.0-1_amd64.deb
```

If you encounter dependency issues, run:

```sh
sudo apt-get install -f
```

### Package contents

The package installs the following files:

- `/usr/lib/by-a-thread/ByAThread` - The game executable
- `/usr/lib/by-a-thread/fonts/` - Font files and licenses
- `/usr/lib/by-a-thread/sfx/` - Sound effect files
- `/usr/lib/by-a-thread/images/` - Game texture files
- `/usr/share/icons/hicolor/256x256/apps/by-a-thread.png` - Application icon
- `/usr/share/applications/by-a-thread.desktop` - Desktop file for applications menu
- `/usr/share/doc/by-a-thread/LICENSE` - Game license
- `/usr/share/doc/by-a-thread/CREDITS.md` - Asset credits and licenses

After installation, the game will be available in your applications menu and can be run from anywhere with `/usr/lib/by-a-thread/ByAThread`.
