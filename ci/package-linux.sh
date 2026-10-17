#!/usr/bin/env bash
# Wraps the Linux binary in an AppImage. Nothing else needs bundling: iced/winit/wgpu
# load X11, Wayland, Vulkan and GL at runtime through dlopen.
set -euo pipefail
source "$(dirname "$0")/lib.sh"
BIN="${1:?binary}"; OUT="${2:?output .AppImage}"
APPDIR="$ROOT/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp "$BIN" "$APPDIR/usr/bin/void-launcher"
chmod 755 "$APPDIR/usr/bin/void-launcher"
cp "$ROOT/packaging/linux/void-launcher.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/void-launcher.png"
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/void-launcher.png" "$APPDIR/void-launcher.png"
ln -sf void-launcher.png "$APPDIR/.DirIcon"
{ cat "$ROOT/packaging/linux/void-launcher.desktop"; echo "X-AppImage-Version=$VERSION"; } > "$APPDIR/void-launcher.desktop"
cp "$APPDIR/void-launcher.desktop" "$APPDIR/usr/share/applications/"
ln -sf usr/bin/void-launcher "$APPDIR/AppRun"
if ! command -v appimagetool >/dev/null; then
  curl -fsSL -o /tmp/appimagetool https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x /tmp/appimagetool
  APPIMAGETOOL=/tmp/appimagetool
else
  APPIMAGETOOL=appimagetool
fi
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" "$APPDIR" "$OUT"
chmod 755 "$OUT"
rm -rf "$APPDIR"
ls -la "$OUT"
