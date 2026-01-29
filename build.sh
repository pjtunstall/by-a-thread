#!/usr/bin/env bash
# Run from the workspace root.
#
# Prerequisites:
#   Rust: https://www.rust-lang.org/tools/install/
#   Make the script executable: chmod +x build.sh
#
# Usage:
#   ./build.sh [--hetzner|-h]
#
set -e

cleanup() {
  rm -rf ByAThread.AppDir ByAThread-win64
}
trap 'cleanup; exit 1' ERR

HETZNER=false
for arg in "$@"; do
  if [ "$arg" = "--hetzner" ] || [ "$arg" = "-h" ]; then
    HETZNER=true
    break
  fi
done

mkdir -p dist

# --- Run tests ---
echo "Running tests..."
cargo test --workspace
echo "Tests passed."
echo

# --- Compile game server ---
echo "Compiling game server..."
cargo build --release -p server
echo "Game server compiled."
echo

# --- Build Docker image of game server ---
#
# Prerequisites:
#   Docker: https://docs.docker.com/engine/install
#
echo "Building Docker image of server..."

# Extract the version number to tag the image with.
VERSION=$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2)

# Tag the image with the version number and as "latest".
docker build \
  -t server-image:$VERSION \
  -t server-image:latest \
  .

echo "Created server-image:$VERSION and server-image:latest"
echo

if [ "$HETZNER" = true ]; then
# --- Update game server on VPS ---
#
# Prerequisites:
#   Ensure that the VPS is running.
#
echo "Pushing Docker image to VPS..."

docker save server-image | gzip | ssh hetzner 'gunzip | docker load'

# --- Run the server container ---
#
# Prerequisites:
#    Ensure that the user who runs this script
#    has permission to run docker commands on the VPS:
#    sudo usermod -aG docker $USER
#
# Stop and remove any existing container first.
# Then run the container.
# (-e IP=... sets the game server's public IP
# to that of the VPS from Hetzner metadata.)
ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm -e IP=$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'

echo "Game server updated on VPS."
echo
fi

# --- Windows executable and zip ---
#
# Prerequisites:
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install mingw-w64 zip
#
echo "Building Windows executable and zip..."

STAGING=ByAThread-win64
EXE=target/x86_64-pc-windows-gnu/release/ByAThread.exe

cargo build --release --target x86_64-pc-windows-gnu -p client

mkdir -p "$STAGING"
cp "$EXE" "$STAGING/"
cp LICENSE CREDITS.md "$STAGING/"
cp client/assets/fonts/LICENSE.txt "$STAGING/NOTO_FONT_LICENSE.txt"
zip -r dist/ByAThread-win64.zip "$STAGING"
rm -r "$STAGING"

echo "Created dist/ByAThread-win64.zip"
echo

# --- Debian .deb package ---
#
# Prerequisites:
#   cargo install cargo-deb
#
# In case of dependency issues, run:
#   sudo apt-get install -f
#
echo "Building Debian .deb package..."
(cd client && cargo build --release && cargo deb)
cp target/debian/by-a-thread_*.deb dist/
echo "Created dist/by-a-thread_*.deb"
echo

# --- RPM package ---
#
# Prerequisites:
#   cargo install cargo-generate-rpm
#
# When installing the .rpm on Fedora/RHEL/openSUSE, if rpm -i fails due to
# missing dependencies, install with dnf/yum instead so they resolve deps:
#   sudo dnf install dist/by-a-thread-*.rpm
#   # or: sudo yum install dist/by-a-thread-*.rpm
#
echo "Building RPM package..."
cargo generate-rpm -p client --payload-compress gzip
cp target/generate-rpm/*.rpm dist/
echo "Created dist/*.rpm"
echo

# --- AppImage ---
#
# Prerequisites:
#   linuxdeploy (e.g. linuxdeploy-x86_64.AppImage) in PATH or set LINUXDEPLOY
#   appimagetool in PATH: https://appimage.github.io/appimagetool/
#
echo "Building AppImage..."
APPDIR=ByAThread.AppDir
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/assets"
cp target/release/ByAThread "$APPDIR/usr/bin/"
cp -r client/assets/fonts client/assets/images client/assets/sfx "$APPDIR/assets/"
cp client/icon.png "$APPDIR/ByAThread.png"
cp client/by-a-thread-appimage.desktop "$APPDIR/ByAThread.desktop"
LINUXDEPLOY="${LINUXDEPLOY:-linuxdeploy}"
"$LINUXDEPLOY" --appdir "$APPDIR" --executable "$APPDIR/usr/bin/ByAThread" --desktop-file "$APPDIR/ByAThread.desktop" --icon-file "$APPDIR/ByAThread.png" 2>&1 | grep -v -e 'WARNING: Could not find copyright' -e 'AppStream upstream metadata is missing' || true
[ "${PIPESTATUS[0]}" -ne 0 ] && exit "${PIPESTATUS[0]}"
appimagetool "$APPDIR" dist/ByAThread.AppImage 2>&1 | grep -v -e 'WARNING: Could not find copyright' -e 'AppStream upstream metadata is missing' || true
[ "${PIPESTATUS[0]}" -ne 0 ] && exit "${PIPESTATUS[0]}"
rm -rf "$APPDIR"
echo "Created dist/ByAThread.AppImage"
echo
