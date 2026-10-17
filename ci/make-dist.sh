#!/usr/bin/env bash
# Assembles dist/: artifacts, SHA256SUMS, manifest.json (read by the launcher's
# self-updater) and build.env (for later jobs).
set -euo pipefail
source "$(dirname "$0")/lib.sh"
cd "$ROOT"
rm -rf dist && mkdir -p dist
declare -A FILES=(
  [windows-x86_64]="void-launcher-windows-x86_64.exe:exe"
  [linux-x86_64]="void-launcher-linux-x86_64:binary"
  [linux-x86_64-appimage]="void-launcher-linux-x86_64.AppImage:appimage"
  [macos-universal]="void-launcher-macos-universal.app.zip:app-zip"
)
MANIFEST_FILES='{}'
for key in "${!FILES[@]}"; do
  name="${FILES[$key]%%:*}"; kind="${FILES[$key]##*:}"
  [ -f "out/$name" ] || { echo "skipping $key: out/$name missing"; continue; }
  cp "out/$name" "dist/$name"
  sha="$(sha256sum "dist/$name" | cut -d' ' -f1)"
  size="$(stat -c %s "dist/$name")"
  MANIFEST_FILES="$(jq --arg k "$key" --arg n "$name" --arg s "$sha" --argjson z "$size" --arg t "$kind" \
    '. + {($k): {name: $n, sha256: $s, size: $z, kind: $t}}' <<<"$MANIFEST_FILES")"
done
REPO="${GITHUB_REPOSITORY:-2011Scape/void-launcher}"
NOTES_URL="https://github.com/$REPO/releases"
if [ "$CHANNEL" = "stable" ]; then NOTES_URL="$NOTES_URL/tag/v$VERSION"; else NOTES_URL="$NOTES_URL/tag/dev"; fi
jq -n --arg v "$VERSION" --arg ch "$CHANNEL" --arg c "$SHORT_SHA" --arg b "$BUILT_AT" --arg notes "$NOTES_URL" --argjson files "$MANIFEST_FILES" \
  '{schema: 1, version: $v, channel: $ch, commit: $c, built_at: $b, min_launcher_version: "1.0.0", notes_url: $notes, files: $files}' > dist/manifest.json
( cd dist && sha256sum void-launcher-* > SHA256SUMS )
printf 'VERSION=%s\nCHANNEL=%s\n' "$VERSION" "$CHANNEL" > dist/build.env
cat dist/manifest.json
ls -la dist
