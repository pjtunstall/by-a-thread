# Installation

- [Overview](#overview)
- [Building the .deb package](#building-the-deb-package)
- [Installing the package](#installing-the-package)
- [Package contents](#package-contents)

## Overview

This document describes how to build and install the Debian package for By a Thread on Linux systems. The package includes the game binary, assets, icon, and desktop file for easy installation.

## Building the .deb package

To create the Debian package, you need to build from the client directory:

```bash
cd client
cargo build --release --bin ByAThread
cargo deb
```

The package will be created at `../target/debian/by-a-thread_0.1.0-1_amd64.deb`.

### Why the `-1` suffix?

The `-1` in the filename is the Debian package revision number. It indicates this is the first revision of version 0.1.0. If you make changes to the package without changing the version number, you would increment this to `-2`, `-3`, etc.

## Installing the package

From the workspace root, install the package using `dpkg`:

```bash
sudo dpkg -i target/debian/by-a-thread_0.1.0-1_amd64.deb
```

If you encounter dependency issues, run:

```bash
sudo apt-get install -f
```

## Package contents

The package installs the following files:

- `/usr/lib/by-a-thread/ByAThread` - The game executable
- `/usr/lib/by-a-thread/fonts/` - Font files
- `/usr/lib/by-a-thread/sfx/` - Sound effect files
- `/usr/share/icons/hicolor/256x256/apps/by-a-thread.png` - Application icon
- `/usr/share/applications/by-a-thread.desktop` - Desktop file for applications menu

After installation, the game will be available in your applications menu and can be run from anywhere with `/usr/lib/by-a-thread/ByAThread`.
