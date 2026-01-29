#!/usr/bin/env bash
# Run from the workspace root.
#
set -e

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
#    has permission to rundocker commands on the VPS:
#    sudo usermod -aG docker $USER
#
# Stop and remove any existing container first.
# Then run the container.
# (-e IP=... sets the game server's public IP
# to that of the VPSfrom Hetzner metadata.)
ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm -e IP=$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'

# --- Windows executable and zip ---
#
# Prerequisites:
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install mingw-w64 zip
#
echo "Building Windows executable and zip..."

STAGING=ByAThread-win64
EXE=client/target/x86_64-pc-windows-gnu/release/ByAThread.exe

cargo build --release --target x86_64-pc-windows-gnu -p client

mkdir -p "$STAGING"
cp "$EXE" "$STAGING/"
cp LICENSE CREDITS.md "$STAGING/"
cp client/assets/fonts/LICENSE.txt "$STAGING/NOTO_FONT_LICENSE.txt"
zip -r ByAThread-win64.zip "$STAGING"
rm -r "$STAGING"

echo "Created ByAThread-win64.zip"

# --- Debian .deb package ---
#
# Prerequisites:
#   cargo install cargo-deb
#
# Build from the client directory so cargo-deb finds package.metadata.deb (see docs/installation.md).
#
echo "Building Debian .deb package..."

(cd client && cargo build --release && cargo deb)

echo "Created client/target/debian/by-a-thread_*.deb"
