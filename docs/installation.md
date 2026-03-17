# Installation

- [Windows](#windows)
- [macOS](#macos)
- [Linux](#linux)

## Windows

Unzip and open the folder, then double click on the .exe. Agree to extract. Click on `ByAThread.exe` to launch the game.

The first time, Windows Defender SmartScreen may open a popup saying it prevented an unrecognised app from starting. Click on "More info" and then "Run anyway".

## macOS

Unzip, then right-click (or Control-click) `ByAThread.app`and select "Open" from the menu. A security prompt will appear. Click the "Open" button inside this prompt to free the app from "quarantine". From now on, you should be able to launch it just by double clicking.

## Linux

Prefer the Debian and RMP options over AppImage if your distro supports them.

### Debian

Open a terminal, navigate to the folder you downloaded it into, e.g. enter `cd ~/Downloads`, then enter `sudo dpkg -i by-a-thread_0.1.0-1_amd64.deb` (this should add `ByAThread` to your applications menu). If you encounter dependency issues, run `sudo apt-get install -f`.

### RPM

As for .deb, except enter `sudo rpm -Uvh by-a-thread-0.1.0-1.x86_64.rpm` (then look for `ByAThread` in your applications menu).

### AppImage

First, download and install AppImageLauncher. Then double click on the ByAThread download and choose integrate. After that, you should have a clickable icon in your applications menu.

If nothing happens the first time, right click on the game and select Properties > Permissions > "Allow executing file as program", and try again.
