#!/bin/sh
# Builds target/Kumo.app from the two release binaries: universal, ad-hoc signed, zipped.
set -e
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
app=target/Kumo.app
rm -rf "$app" target/Kumo.iconset
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" target/Kumo.iconset

lipo -create target/aarch64-apple-darwin/release/kumo target/x86_64-apple-darwin/release/kumo -output "$app/Contents/MacOS/kumo"

for size in 16 32 128 256 512; do
    sips -z "$size" "$size" ../assets/kumo-icon.png --out "target/Kumo.iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" ../assets/kumo-icon.png --out "target/Kumo.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns target/Kumo.iconset -o "$app/Contents/Resources/Kumo.icns"
cp ../LICENSE "$app/Contents/Resources/LICENSE"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Kumo</string>
    <key>CFBundleDisplayName</key><string>Kumo</string>
    <key>CFBundleIdentifier</key><string>tech.hyos.kumo</string>
    <key>CFBundleExecutable</key><string>kumo</string>
    <key>CFBundleIconFile</key><string>Kumo</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>GPL-3.0</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app"
(cd target && ditto -c -k --keepParent Kumo.app Kumo-mac.zip)
du -sh "$app" target/Kumo-mac.zip
lipo -info "$app/Contents/MacOS/kumo"
