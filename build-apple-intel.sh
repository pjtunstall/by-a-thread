#!/usr/bin/env bash
# Build the client for Intel Mac (x86_64) and create a .app bundle.
# Run from the workspace root on macOS.
#
# Prerequisites:
#   Rust: https://www.rust-lang.org/tools/install/
#   rustup target add x86_64-apple-darwin
#   For the app icon: create client/icon.icns (e.g. from icon.png using iconutil on macOS).
#
# Usage:
#   ./build-apple-intel.sh
#
set -e

TARGET=x86_64-apple-darwin
APP_NAME=ByAThread
BUNDLE=$APP_NAME.app
STAGING=dist/$APP_NAME-macos-intel

echo "Building for Intel Mac ($TARGET)..."
rustup target add $TARGET 2>/dev/null || true
cargo build --release --target $TARGET -p client

echo "Creating .app bundle..."
rm -rf "$BUNDLE" "$STAGING"
mkdir -p "$BUNDLE/Contents/MacOS"
mkdir -p "$BUNDLE/Contents/Resources/fonts"
mkdir -p "$BUNDLE/Contents/Resources/images"
mkdir -p "$BUNDLE/Contents/Resources/sfx"

cp target/$TARGET/release/$APP_NAME "$BUNDLE/Contents/MacOS/"
cp client/assets/fonts/"PF Hellenica Serif Pro Bold.ttf" "$BUNDLE/Contents/Resources/fonts/"
cp client/assets/fonts/NotoSerifBold-MmDx.ttf "$BUNDLE/Contents/Resources/fonts/"
cp client/assets/images/*.png "$BUNDLE/Contents/Resources/images/"
cp client/assets/sfx/*.wav "$BUNDLE/Contents/Resources/sfx/"

if [ -f client/icon.icns ]; then
  cp client/icon.icns "$BUNDLE/Contents/Resources/"
  ICON_PLIST='    <key>CFBundleIconFile</key>
    <string>icon</string>
'
else
  ICON_PLIST=""
fi

cat > "$BUNDLE/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.byathread.client</string>
    <key>CFBundleName</key>
    <string>By A Thread</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
${ICON_PLIST}</dict>
</plist>
EOF

mkdir -p dist "$STAGING"
cp -R "$BUNDLE" "$STAGING/"
cp LICENSE CREDITS.md "$STAGING/"
cp client/assets/fonts/LICENSE.txt "$STAGING/NOTO_FONT_LICENSE.txt"
(cd dist && zip -r "${APP_NAME}-macos-intel.zip" "$(basename "$STAGING")")
rm -rf "$STAGING"
echo "Created $BUNDLE and dist/${APP_NAME}-macos-intel.zip"
