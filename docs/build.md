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

This document describes how to create executable files or packages for various systems. The full `make` build assumes you're creating the Linux versions on Ubuntu or a similar (Debian-based) distro. For other Linux distros, the commands for importing dependencies may vary according to your package manager. The Windows version can be built on Linux, but is best built on Windows as, currently, that's been the only way I've managed to get the icon image to display. To build on Windows, use the `Build-Windows.ps1` PowerShell script, and, on Ubuntu, `make windows`. The macOS versions should be built on Apple Intel and Apple Silicon by running `make macos-intel` and `make macos-silicon` respectively. (These last two have yet to be tested.)

From the workspace root you can run the full build with `make`. To build only one artifact, use e.g. `make windows`, `make deb`, `make rpm`, or `make appimage`. To deploy the backend to the VPS, use `make deploy` as described in `docs/devops.md`.

Debug builds (e.g. `cargo run`) default to windowed mode; release builds default to fullscreen. Users who encounter graphics driver issues (e.g. WGL_ARB_pixel_format on Windows) can force windowed mode with `--windowed` or `BY_A_THREAD_WINDOWED=1`.

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

To test that it was created correctly:

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

That said, I've so far been unable to get the `.ico` image to show on the `.exe` except by building it on Windows.

### Building the executable

**On Ubuntu:** Run `make windows` from the workspace root. Prerequisites: `rustup target add x86_64-pc-windows-gnu` and `apt install mingw-w64 zip`. This cross-compiles the client and writes `dist/ByAThread-<version>-win64.zip` (version from the Makefile), with everything under a `ByAThread` directory in the archive.

**On Windows:** Run `.\scripts\Build-Windows.ps1` from the project root. It writes the same style of zip (`dist/ByAThread-<version>-win64.zip`, version from the `client` crate in `cargo metadata`), with the same `ByAThread` layout, containing the Windows executable (with `.ico` as its icon), credits, and licenses.

### Distribution

Ignore virus warnings; that just means the file is from an unknown publisher. If SmartScreen tells you, "Windows has protected your PC", click "info" to reveal the hidden "run anyway" button.

## macOS

Build on macOS using the Makefile:

- `make macos-intel` – Intel Mac (x86_64), produces `dist/ByAThread-macos-intel.zip`
- `make macos-silicon` – Apple Silicon (aarch64), produces `dist/ByAThread-macos-silicon.zip`

Each build compiles the client for the target architecture, then runs `scripts/bundle-macos.sh` to create a .app bundle and zip it. The script assembles `ByAThread.app` with the executable, fonts, images, sounds, and Info.plist; copies it into a staging directory with LICENSE and CREDITS; and zips the result. The .app is double-clickable and shows in the Dock. For the app icon to appear, create `client/icon.icns` (e.g. from `client/icon.png` using `iconutil` on macOS).

macOS builds are done on Mac only (cross-compilation from Linux is not supported). The Makefile uses a shell script rather than inline commands so it works with the default BSD make on macOS.

## Linux

There are three options for Linux: `.deb` and `.rpm` according to Linux distro type (advantage: native system integration), and AppImage, which bundles the game and its dependencies (libraries and assets) into a single executable file that should be compatible with any distro.

Use the `.deb` on Debian, Ubuntu and other apt-based distros; use the `.rpm` on Fedora, RHEL, openSUSE and other RPM-based distros. On Arch Linux and other distros that use neither format, use the AppImage or build from source.

### Compatibility

The binary's runtime requirements (such as glibc version) are determined by the machine or container you build on. If that environment has a newer glibc than the systems where users will run the game, the binary may fail at runtime. Building on an older Ubuntu (e.g. in CI or a local container) avoids that.

A common solution is to build Linux artifacts (`.deb`, `.rpm`, AppImage) in an automated run (e.g. GitHub Actions) on an older image such as `ubuntu-22.04` or `ubuntu-20.04`, so the binaries link against an older glibc and run on a wide range of distros. You can run `make` locally on any supported Linux for testing; for published releases, the GitHub Actions workflows build these artifacts in a fixed Ubuntu environment and upload them for distribution.

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
- `/usr/lib/by-a-thread/fonts/macondo/` - Macondo font (`Macondo-Regular.ttf`) and SIL Open Font License (`OFL.txt`)
- `/usr/lib/by-a-thread/fonts/noto/` - Map subset font (`NotoSubset.ttf`) and Apache 2.0 license (`LICENSE.txt`)
- `/usr/lib/by-a-thread/sfx/` - Sound effect files
- `/usr/lib/by-a-thread/images/` - Game texture files
- `/usr/share/icons/hicolor/256x256/apps/by-a-thread.png` - Application icon
- `/usr/share/applications/by-a-thread.desktop` - Desktop file for applications menu
- `/usr/share/doc/by-a-thread/LICENSE` - Game license
- `/usr/share/doc/by-a-thread/CREDITS` - Asset credits and licenses

After installation, the game will be available in your applications menu and can be run from anywhere with `/usr/lib/by-a-thread/ByAThread` or by clicking on the icon in your taskbar.

Note that game client instances will appear as a plain (cogwheel) icons in the taskbar, instead of a dot beside the icon you clicked. I gather this is because Macroquad, the library I used for window management, doesn't support full taskbar integration.

### .deb

Built by the full `make` target (or `make deb` alone). Prerequisite: `cargo install cargo-deb`.

The build runs `cargo deb -p client` and copies the resulting `.deb` into `dist/`. Names follow the usual Debian binary package form:

`by-a-thread_<upstream_version>-<debian_revision>_<architecture>.deb`

- **Package name** `by-a-thread` comes from `[package.metadata.deb]` in `client/Cargo.toml`.
- **`<upstream_version>`** is the workspace version (same across crates).
- **`<debian_revision>`** is the `revision` field in that metadata. This repo sets `1`; bump if you repackage without changing the upstream version. Note: Debian calls the **Debian revision**; it matches the role of **`release`** in an RPM (below).
- **`<architecture>`** is typically `amd64` for these builds.

Example: `by-a-thread_0.1.0-1_amd64.deb`.

From the workspace root:

```sh
sudo dpkg -i dist/by-a-thread_*.deb
```

If dependency resolution fails:

```sh
sudo apt-get install -f
```

### .rpm

Built by the full `make` target (or `make rpm` alone). Prerequisite: `cargo install cargo-generate-rpm` (the binary is invoked as `cargo generate-rpm`).

The build runs `cargo generate-rpm -p client` (with gzip payload compression, per the `Makefile`) and copies the `.rpm` into `dist/`. Names follow the usual RPM form:

`by-a-thread-<version>-<release>.x86_64.rpm`

- **`<version>`** is the workspace package version.
- **`<release>`** is the `release` field under `[package.metadata.generate-rpm]` in `client/Cargo.toml`. This repo sets `1`; bump when repackaging the same upstream version. Note: RPM `.spec` files call those fields **Version** and **Release**; **Release** here is the same idea as Debian **revision** (above).

Example: `by-a-thread-0.1.0-1.x86_64.rpm`.

From the workspace root:

```sh
sudo rpm -i dist/by-a-thread-*.rpm
```

On Fedora and similar:

```sh
sudo dnf install dist/by-a-thread-*.rpm
```

That compression and metadata layout target current Fedora, openSUSE, RHEL 8+, and similar. CentOS 7 and other RPMv3-only environments are not supported by cargo-generate-rpm.

GNOME, KDE, and other desktops typically show the launcher from an installed `.deb` or `.rpm`. After you rebuild locally, reinstall from `dist/` so the menu entry runs the new binary. AppImages (below) do not register in the menu until integrated; [installation.md](installation.md#appimage) recommends **AppImageLauncher** for that.

### AppImage

Built by the full `make` target (or `make appimage` alone). You also need **linuxdeploy** and **appimagetool** on your `PATH`; installation steps appear later in this section.

The `Makefile` writes a single file to `dist/`:

`ByAThread-<version>.AppImage`

**`<version>`** is the `client` crate version from `cargo metadata` (the workspace version in the root `Cargo.toml`).

Example: `ByAThread-0.1.0.AppImage`.

The binary runs without a full install: users can just mark it executable and launch it, or install with **AppImageLauncher** (recommended in [installation.md](installation.md)) so the desktop lists it like a normal app. Both approaches are valid; AppImageLauncher mainly improves menu integration and updates when you drop in a newer file.

There is only one build file specific to AppImage:

- `client/by-a-thread-appimage.desktop`

**What the build does.** It builds the AppImage in two stages. First it assembles a folder (an **AppDir**, the standard name for "a folder containing the app and its files before it's turned into an AppImage"). The build uses the folder `ByAThread.AppDir`. It copies the binary, assets, icon, and `client/by-a-thread-appimage.desktop` into it; the `.desktop` file is written into the AppDir as `ByAThread.desktop` because the AppDir layout expects a `.desktop` file named after the app. Then it runs **linuxdeploy** (which adds the launcher script and bundled libraries) and **appimagetool** (which turns the folder into the single `ByAThread.AppImage` file). Then it deletes the temporary folder. You never need to create or edit the AppDir yourself.

**Why two `.desktop` files?** The .deb and .rpm install under `/usr`, so `by-a-thread.desktop` uses paths like `/usr/lib/by-a-thread/ByAThread`. Inside an AppImage there's no `/usr` install; the binary is just `ByAThread` in the image's path. So we use a second file, `by-a-thread-appimage.desktop`, with `Exec=ByAThread` and `Icon=ByAThread`. The build copies that into the AppDir when building.

**Prerequisites (what you must do before running the build).** The build needs two tools: **linuxdeploy** and **appimagetool**. Both are distributed as AppImages. For local and CI use, install them the same way: download the AppImages, make them executable, put them in a directory that is in your PATH (e.g. `~/bin` or `/usr/local/bin`), and create symlinks so the build can run them by name (`linuxdeploy` and `appimagetool`). Then `make` will find them.

1. Download [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy/releases) (`linuxdeploy-x86_64.AppImage`) and [appimagetool](https://github.com/AppImage/appimagetool/releases) (or [appimage.github.io/appimagetool](https://appimage.github.io/appimagetool/)). For linuxdeploy, prefer a versioned release (e.g. the latest `1-alpha-...`) over **continuous** so builds are reproducible, especially in CI; continuous is fine for one-off local use.
2. `chmod +x linuxdeploy-x86_64.AppImage appimagetool-*.AppImage`
3. Put both in a PATH directory and symlink: `ln -s /path/to/linuxdeploy-x86_64.AppImage ~/.local/bin/linuxdeploy` and `ln -s /path/to/appimagetool-*.AppImage ~/bin/appimagetool`. If `~/.local/bin` is not in your PATH, add `export PATH="$HOME/bin:$PATH"` to `~/.bashrc` (if in a local shell) or `~/.profile` (if SSH). Then `make` will find them:

```sh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

(To use linuxdeploy from a different path without putting it in PATH, set the environment variable `LINUXDEPLOY` to the full path of the file when you run `make`; `appimagetool` must still be in PATH.)

When you download the AppImage from itch.io, it should already be marked as executable by the CI workflow. For an app-menu entry, follow [installation.md](installation.md): install **AppImageLauncher** and integrate when prompted, or use another distro-specific helper if you prefer.
