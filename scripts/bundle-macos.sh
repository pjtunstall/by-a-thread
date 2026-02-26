#!/usr/bin/env bash
set -e

TARGET=$1
STAGING=$2
ZIP_NAME=$3

if [ -z "$TARGET" ] || [ -z "$STAGING" ] || [ -z "$ZIP_NAME" ]; then
    exit 1
fi

echo "Building macOS bundle: $ZIP_NAME"
BUNDLE=ByAThread.app

rm -rf "$BUNDLE" "dist/$STAGING"
mkdir -p "$BUNDLE/Contents/MacOS" \
         "$BUNDLE/Contents/Resources/assets/fonts" \
         "$BUNDLE/Contents/Resources/assets/images" \
         "$BUNDLE/Contents/Resources/assets/sfx"

cat > "$BUNDLE/Contents/MacOS/launcher.sh" << 'EOF'
#!/usr/bin/env bash
cd "$(dirname "$0")/../Resources"
exec "../MacOS/ByAThread"
EOF
chmod +x "$BUNDLE/Contents/MacOS/launcher.sh"

cp "target/$TARGET/release/ByAThread" "$BUNDLE/Contents/MacOS/"
cp "client/assets/fonts/PF Hellenica Serif Pro Bold.ttf" \
   "client/assets/fonts/NotoSerifBold-MmDx.ttf" \
   "$BUNDLE/Contents/Resources/assets/fonts/"
cp client/assets/images/*.png "$BUNDLE/Contents/Resources/assets/images/"
cp client/assets/sfx/*.wav "$BUNDLE/Contents/Resources/assets/sfx/"

if [ -f client/icon.icns ]; then
    cp client/icon.icns "$BUNDLE/Contents/Resources/"
    ICON_PLIST='
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>'
else
    ICON_PLIST=
fi

cat > "$BUNDLE/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>launcher.sh</string>
    <key>CFBundleIdentifier</key>
    <string>com.byathread.client</string>
    <key>CFBundleName</key>
    <string>By a Thread</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>${ICON_PLIST}
</dict>
</plist>
PLIST

codesign -s - -f --deep "$BUNDLE"

mkdir -p dist
cp -R "$BUNDLE" "dist/$STAGING/"
cp LICENSE CREDITS "dist/$STAGING/"
cp client/assets/fonts/LICENSE.txt "dist/$STAGING/NOTO_FONT_LICENSE.txt"
(cd dist && zip -r "$ZIP_NAME" "$STAGING")
rm -rf "dist/$STAGING"
rm -rf "$BUNDLE"