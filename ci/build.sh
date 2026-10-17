#!/usr/bin/env bash
# Builds one target on its native runner and drops the packaged result(s) into out/.
#   ci/build.sh linux-x86_64 | windows-x86_64 | macos-universal
set -euo pipefail
source "$(dirname "$0")/lib.sh"
TARGET_NAME="${1:?target name}"
cd "$ROOT"
mkdir -p out
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"

case "$TARGET_NAME" in
  linux-x86_64)
    cargo build --release --locked -p void-launcher
    cp target/release/void-launcher out/void-launcher-linux-x86_64
    ci/check-binary.sh linux out/void-launcher-linux-x86_64
    ci/package-linux.sh out/void-launcher-linux-x86_64 out/void-launcher-linux-x86_64.AppImage
    ;;
  windows-x86_64)
    cargo build --release --locked -p void-launcher
    cp target/release/void-launcher.exe out/void-launcher-windows-x86_64.exe
    ci/check-binary.sh windows out/void-launcher-windows-x86_64.exe
    ;;
  macos-universal)
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-10.15}"
    rustup target add x86_64-apple-darwin aarch64-apple-darwin
    cargo build --release --locked -p void-launcher --target x86_64-apple-darwin
    cargo build --release --locked -p void-launcher --target aarch64-apple-darwin
    cp target/x86_64-apple-darwin/release/void-launcher out/void-launcher-x86_64-apple-darwin
    cp target/aarch64-apple-darwin/release/void-launcher out/void-launcher-aarch64-apple-darwin
    ci/check-binary.sh macos out/void-launcher-x86_64-apple-darwin
    ci/check-binary.sh macos out/void-launcher-aarch64-apple-darwin
    ci/package-macos.sh
    ;;
  *) echo "unknown target $TARGET_NAME" >&2; exit 2 ;;
esac
ls -la out
