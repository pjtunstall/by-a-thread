#!/usr/bin/env bash
# Run from workspace root. Creates ByAThread.app and dist/ZIP_NAME.
# Usage: macos-bundle.sh TARGET STAGING_DIR ZIP_NAME
# e.g.   macos-bundle.sh aarch64-apple-darwin ByAThread-macos-silicon ByAThread-macos-silicon.zip
set -e

TARGET=$1
STAGING=$2
ZIP_NAME=$3

if [ -z "$TARGET" ] || [ -z "$STAGING" ] || [ -z "$ZIP_NAME" ]; then
    echo "Usage: $0 TARGET STAGING_DIR ZIP_NAME" >&2
    exit 1
fi

BUNDLE=ByAThread.app

rm -rf "$BUNDLE" "dist/$STAGING"
mkdir -p "$BUNDLE/Contents/MacOS" \
         "$BUNDLE/Contents/Resources/fonts" \
         "$BUNDLE/Contents/Resources/images" \
         "$BUNDLE/Contents/Resources/sfx"

cp "target/$TARGET/release/ByAThread" "$BUNDLE/Contents/MacOS/"
cp "client/assets/fonts/PF Hellenica Serif Pro Bold.ttf" \
   "client/assets/fonts/NotoSerifBold-MmDx.ttf" \
   "$BUNDLE/Contents/Resources/fonts/"
cp client/assets/images/*.png "$BUNDLE/Contents/Resources/images/"
cp client/assets/sfx/*.wav "$BUNDLE/Contents/Resources/sfx/"
if [ -f client/icon.icns ]; then
    cp client/icon.icns "$BUNDLE/Contents/Resources/"
fi

if [ -f client/icon.icns ]; then
    ICON_PLIST='
    <key>CFBundleIconFile</key>
    <string>icon</string>'
else
    ICON_PLIST=
fi

cat > "$BUNDLE/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>ByAThread</string>
    <key>CFBundleIdentifier</key>
    <string>com.byathread.client</string>
    <key>CFBundleName</key>
    <string>By A Thread</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>${ICON_PLIST}
</dict>
</plist>
PLIST

mkdir -p dist
cp -R "$BUNDLE" "dist/$STAGING/"
cp LICENSE CREDITS.md "dist/$STAGING/"
cp client/assets/fonts/LICENSE.txt "dist/$STAGING/NOTO_FONT_LICENSE.txt"
(cd dist && zip -r "$ZIP_NAME" "$STAGING")
rm -rf "dist/$STAGING"
rm -rf "$BUNDLE"
