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
- `icon.ico` - Icons in various sizes
- `Cargo.toml` sections:
  - `[build-dependencies]` with `embed-resource = "3.0.6"`
  - `[[bin]]` section defining the `ByAThread` binary

The `.ico` file was built from the PNG with:

```sh
convert icon.png -define icon:auto-resize="256,128,96,64,48,32,24,16" icon.ico
```

To test that it worked:

```sh
file icon.ico
```

Expected output:

```sh
icon.ico[0] PNG 256x256 256x256+0+0 8-bit sRGB 23680B 0.000u 0:00.002
icon.ico[1] ICO 128x128 128x128+0+0 8-bit sRGB 0.000u 0:00.002
icon.ico[2] ICO 96x96 96x96+0+0 8-bit sRGB 0.000u 0:00.001
icon.ico[3] ICO 64x64 64x64+0+0 8-bit sRGB 0.000u 0:00.001
icon.ico[4] ICO 48x48 48x48+0+0 8-bit sRGB 0.000u 0:00.000
icon.ico[5] ICO 32x32 32x32+0+0 8-bit sRGB 0.000u 0:00.000
icon.ico[6] ICO 24x24 24x24+0+0 8-bit sRGB 0.000u 0:00.000
icon.ico[7] ICO 16x16 16x16+0+0 8-bit sRGB 163902B 0.000u 0:00.000
```

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
