#!/usr/bin/env bash
# lipo the two macOS slices into a universal binary, wrap it in Void.app,
# ad-hoc sign it (required on Apple Silicon) and zip it. Runs on a macOS runner.
set -euo pipefail
source "$(dirname "$0")/lib.sh"
cd "$ROOT"
X64=out/void-launcher-x86_64-apple-darwin
ARM=out/void-launcher-aarch64-apple-darwin
APP=out/Void.app
OUT=out/void-launcher-macos-universal.app.zip
rm -rf "$APP" "$OUT"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create "$X64" "$ARM" -output "$APP/Contents/MacOS/void-launcher"
chmod 755 "$APP/Contents/MacOS/void-launcher"
cp packaging/macos/void.icns "$APP/Contents/Resources/void.icns"
BUNDLE_VERSION="${GITHUB_RUN_NUMBER:-1}"
sed -e "s/@VERSION@/${VERSION%%-*}/g" -e "s/@BUILD@/$BUNDLE_VERSION/g" packaging/macos/Info.plist.in > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"
# Ad-hoc signature: unsigned arm64 binaries are killed by the kernel on Apple Silicon.
codesign --force --deep --sign - --options runtime "$APP"
codesign --verify --verbose=2 "$APP"
( cd out && ditto -c -k --keepParent Void.app "$(basename "$OUT")" )
rm -rf "$APP"
ls -la "$OUT"
