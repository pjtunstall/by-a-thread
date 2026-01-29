# Build

- [Overview](#overview)
- [Windows](#windows)
  - [Build files](#build-files)
  - [Icon](#icon)
  - [Distribution](#distribution)
- [macOS](#macos)
- [Linux](#linux)
  - [Build files](#build-files)
  - [Package contents](#package-contents)
  - [.deb](#deb)
  - [.rpm](#rpm)

## Overview

This document describes how to create executable files or packages for various systems. It assumes you're creating them on Linux.

## Windows

When you run the general build script, `build.sh`, it produces a `.zip` file, containing a Windows executable file, credits, and licenses.

### Build files

Specific to the Windows build process are these components of the `client` directory:

- `src/build.rs` - Build script that compiles the icon resource
- `icon.rc` - Resource file specifying the icon to embed
- `icon.ico` - Icons in various sizes
- `Cargo.toml` sections:
  - `[build-dependencies]` with `embed-resource = "3.0.6"`
  - `[[bin]]` section defining the `ByAThread` binary

### Icon

The `.ico` file was built from the PNG using ImageMagick with:

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

### Distribution

Ignore virus warnings; that just means the file is from an unknown publisher. If SmartScreen tells you, "Windows has protected your PC", click "info" to reveal the hidden "run anyway" button.

## macOS

Build on macOS using the scripts in the project root:

- `./build-apple-intel.sh` – Intel Mac (x86_64), produces `ByAThread.app` and `dist/ByAThread-macos-intel.zip`
- `./build-apple-silicon.sh` – Apple Silicon (aarch64), produces `ByAThread.app` and `dist/ByAThread-macos-silicon.zip`

Each script creates a .app bundle so the app is double-clickable and shows in the Dock. For the app icon to appear, create `client/icon.icns` (e.g. from `client/icon.png` using `iconutil` on macOS).

## Linux

### Build files

The Linux package builds (.deb and .rpm) use these components of the `client` directory:

- `icon.png` - Icon file for the application
- `by-a-thread.desktop` - Desktop file for applications menu
- `Cargo.toml` sections:
  - `[package.metadata.deb]` and `[package.metadata.generate-rpm]` with package metadata and asset paths
  - `[[bin]]` section defining the `ByAThread` binary

### Package contents

Both the .deb and .rpm packages install the following files:

- `/usr/lib/by-a-thread/ByAThread` - The game executable
- `/usr/lib/by-a-thread/fonts/` - Font files and licenses
- `/usr/lib/by-a-thread/sfx/` - Sound effect files
- `/usr/lib/by-a-thread/images/` - Game texture files
- `/usr/share/icons/hicolor/256x256/apps/by-a-thread.png` - Application icon
- `/usr/share/applications/by-a-thread.desktop` - Desktop file for applications menu
- `/usr/share/doc/by-a-thread/LICENSE` - Game license
- `/usr/share/doc/by-a-thread/CREDITS.md` - Asset credits and licenses

After installation, the game will be available in your applications menu and can be run from anywhere with `/usr/lib/by-a-thread/ByAThread` or by clicking on the icon in your taskbar.

Note that game client instances will appear as a plain (cogwheel) icons in the taskbar, instead of a dot beside the icon you clicked. I gather this is because Macroquad, the library I used for window management, doesn't support full taskbar integration.

### .deb

The `.deb` package is built as one step of the general `build.sh` script. Prerequisite: `cargo install cargo-deb`.

The `-1` in the filename is the Debian package revision number. It indicates this is the first revision of version 0.1.0. If you make changes to the package without changing the version number, you would increment this to `-2`, `-3`, etc.

To install: if you're still in `client` folder, move back to the workspace root, then:

```sh
sudo dpkg -i dist/by-a-thread_*.deb
```

If you encounter dependency issues, run:

```sh
sudo apt-get install -f
```

### .rpm

The `.rpm` package is built as one step of the general `build.sh` script. Prerequisite: `cargo install cargo-generate-rpm`. The script produces a file such as `by-a-thread-0.1.0-1.x86_64.rpm` in `dist/`, for use on Fedora, RHEL, openSUSE and other RPM-based distributions.

To install from the workspace root:

```sh
sudo rpm -i dist/by-a-thread-*.rpm
```

Or, on Fedora and similar:

```sh
sudo dnf install dist/by-a-thread-*.rpm
```
