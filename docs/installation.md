# Installation

- [Windows](#windows)
- [macOS](#macos)
- [Linux](#linux)
  - [Debian](#debian)
  - [RPM](#rpm)
  - [AppImage](#appimage)

This guide assumes you downloaded the client from the [website](https://by-a-thread.de). If your download came from itch.io, substitute the name of the file you got from there (as they follow a different convention).

## Windows

Extract the zip, open the **ByAThread** folder, and launch `ByAThread.exe`.

On the first launch, SmartScreen may block it; click **More info**, then **Run anyway**.

(The default is to treat any download that hasn't been registered with Microsoft as a virus till you confirm otherwise.)

## macOS

Double-click the archive to extract it if needed. Drag `ByAThread.app` into **Applications** if you want it there (optional).

Downloads from the browser get a **quarantine** flag, so **Gatekeeper** may stop a plain double-click until you confirm you trust the app.

**First launch (recommended):** Right-click `ByAThread.app`, choose **Open**, then click **Open** in the dialog. After that, a normal double-click should work.

**If it's still blocked:** Open **System Settings** > **Privacy & Security** and allow **ByAThread**. On macOS 12 and earlier, use **System Preferences** > **Security & Privacy** instead.

## Linux

Prefer the Debian or RPM options over AppImage if your distro supports them.

### Debian

Run `sudo apt install ./ByAThread-linux.deb`, then launch the game from your app menu.

### RPM

Install with your distro tools when you can, for example `sudo dnf install ./ByAThread-linux.rpm` on Fedora or `sudo zypper install ./ByAThread-linux.rpm` on openSUSE (they resolve dependencies like `apt`).

Otherwise use `sudo rpm -Uvh ./ByAThread-linux.rpm`. (`-U` installs a new package or upgrades one that is already installed, while `rpm -i` errors if that package is already installed.)

Then launch the game from your app menu.

### AppImage

You have two options here. I recommend first installing **AppImageLauncher**. It's not required for AppImages to run, but many desktops treat a bare file as a one-off download; registering it gives you a normal app-menu entry and a clearer path when you drop in a newer build.

**With AppImageLauncher (recommended):** Install **AppImageLauncher**, double-click `ByAThread-linux.AppImage`, choose **Integrate** when prompted, then launch the game from your app menu.

**Without AppImageLauncher:** Mark `ByAThread-linux.AppImage` as executable. In your file manager, right-click the file, open **Properties** > **Permissions**, and enable **Allow executing file as program**. Then double-click to run.
