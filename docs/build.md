# Build

- [Overview](#overview)
- [Windows](#windows)
  - [Building the executable](#building-the-executable)
  - [Distribution](#distribution)
- [macOS](#macos)
- [Linux](#linux)
  - [Compatibility](#compatibility)
  - [Build files](#build-files)
  - [Package contents](#package-contents)
  - [.deb](#deb)
  - [.rpm](#rpm)
  - [AppImage](#appimage)

## Overview

This document describes how to create executable files or packages for various systems. It assumes you're creating them on Ubuntu.

## Windows

### Build files

Specific to the Windows build process are these components of the `client` directory:

- `src/build.rs` - Build script that compiles the icon resource
- `icon.rc` - Resource file specifying the icon to embed
- `icon.ico` - Icons in various sizes
- `Cargo.toml` sections:
  - `[build-dependencies]` with `embed-resource = "3.0.6"`
  - `[[bin]]` section defining the `ByAThread` binary

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

### Building the executable

When you run the general build script, `build.sh`, it produces a `.zip` file, containing a Windows executable file, credits, and licenses.

### Distribution

Ignore virus warnings; that just means the file is from an unknown publisher. If SmartScreen tells you, "Windows has protected your PC", click "info" to reveal the hidden "run anyway" button.

## macOS

Build on macOS using the scripts in the project root:

- `./build-apple-intel.sh` – Intel Mac (x86_64), produces `ByAThread.app` and `dist/ByAThread-macos-intel.zip`
- `./build-apple-silicon.sh` – Apple Silicon (aarch64), produces `ByAThread.app` and `dist/ByAThread-macos-silicon.zip`

Each script creates a .app bundle so the app is double-clickable and shows in the Dock. For the app icon to appear, create `client/icon.icns` (e.g. from `client/icon.png` using `iconutil` on macOS).

## Linux

There are three options for Linux: `.deb` and `.rpm` according to Linux distro type (advantage: native system integration), and AppImage, which bundles the game and its dependencies (libraries and assets) into a single executable file that should be compatible with any distro.

Use the `.deb` on Debian, Ubuntu and other apt-based distros; use the `.rpm` on Fedora, RHEL, openSUSE and other RPM-based distros. On Arch Linux and other distros that use neither format, use the AppImage or build from source.

### Compatibility

The binary is built for the machine (or container) you run `build.sh` on. If that environment has a newer glibc than the systems where users will run the game, the binary may fail at runtime. Building on an older Ubuntu (e.g. in CI or a local container) avoids that.

A common solution is to build Linux artifacts (`.deb`, `.rpm`, AppImage) in an automated run (e.g. GitHub Actions) on an older image such as `ubuntu-22.04` or `ubuntu-20.04`, so the binaries link against an older glibc and run on a wide range of distros. You can run `build.sh` locally on any supported Linux for testing; for published releases, running it inside a container or CI on a fixed older Ubuntu avoids compatibility surprises. A later step is to add a workflow that runs the script in that environment and uploads the artifacts.

### Build files

All three types of Linux package (.deb, .rpm, and AppImage) are built using these components of the `client` directory:

- `icon.png` - Icon file for the application
- `by-a-thread.desktop` - Desktop file for .deb and .rpm (points at the installed path under `/usr`)
- `by-a-thread-appimage.desktop` - Desktop file used only when building the AppImage (different paths, since the AppImage is not installed under `/usr`)
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

The RPM uses gzip payload compression so it can be installed on any recent rpm (Fedora, openSUSE, RHEL 8+, etc.). CentOS 7 and other RPMv3-based systems are not supported by cargo-generate-rpm.

### AppImage

An **AppImage** is a single file that runs on most Linux desktops without installation: the user downloads it, makes it executable, and runs it. The script produces `ByAThread.AppImage` in `dist/`.

There is only one build file specific to AppImage:

- `client/by-a-thread-appimage.desktop`

**What the script does.** It builds the AppImage in two stages. First it assembles a folder (an **AppDir**, the standard name for "a folder containing the app and its files before it's turned into an AppImage"). Our script calls that folder `ByAThread.AppDir`. It copies the binary, assets, icon, and `client/by-a-thread-appimage.desktop` into it; the `.desktop` file is written into the AppDir as `ByAThread.desktop` because the AppDir layout expects a `.desktop` file named after the app. Then it runs **linuxdeploy** (which adds the launcher script and bundled libraries) and **appimagetool** (which turns the folder into the single `ByAThread.AppImage` file). Then it deletes the temporary folder. You never need to create or edit the AppDir yourself.

**Why two `.desktop` files?** The .deb and .rpm install under `/usr`, so `by-a-thread.desktop` uses paths like `/usr/lib/by-a-thread/ByAThread`. Inside an AppImage there's no `/usr` install; the binary is just `ByAThread` in the image's path. So we use a second file, `by-a-thread-appimage.desktop`, with `Exec=ByAThread` and `Icon=ByAThread`. The script copies that into the AppDir when building.

**Prerequisites (what you must do before running the script).** The script needs two tools: **linuxdeploy** and **appimagetool**. Both are distributed as AppImages. For local and CI use, install them the same way: download the AppImages, make them executable, put them in a directory that is in your PATH (e.g. `~/bin` or `/usr/local/bin`), and create symlinks so the script can run them by name (`linuxdeploy` and `appimagetool`). Then `./build.sh` will find them.

1. Download [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy/releases) (`linuxdeploy-x86_64.AppImage`) and [appimagetool](https://github.com/AppImage/appimagetool/releases) (or [appimage.github.io/appimagetool](https://appimage.github.io/appimagetool/)). For linuxdeploy, prefer a versioned release (e.g. the latest `1-alpha-...`) over **continuous** so builds are reproducible, especially in CI; continuous is fine for one-off local use.
2. `chmod +x linuxdeploy-x86_64.AppImage appimagetool-*.AppImage`
3. Put both in a PATH directory and symlink: `ln -s /path/to/linuxdeploy-x86_64.AppImage ~/.local/bin/linuxdeploy` and `ln -s /path/to/appimagetool-*.AppImage ~/bin/appimagetool`. If `~/.local/bin` is not in your PATH, add `export PATH="$HOME/bin:$PATH"` to `~/.bashrc` (if in a local shell) or `~/.profile` (if SSH):

```sh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

(To use linuxdeploy from a different path without putting it in PATH, set the environment variable `LINUXDEPLOY` to the full path of the file when you run the script; `appimagetool` must still be in PATH.)
