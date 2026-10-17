#!/usr/bin/env sh
# Fast guard rails that run before any build.
set -eu
CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
echo "Cargo.toml version: $CARGO_VERSION"
case "${GITHUB_REF:-}" in
  refs/tags/v*)
    TAG="${GITHUB_REF#refs/tags/}"
    if [ "${TAG#v}" != "$CARGO_VERSION" ]; then
      echo "FAIL: tag $TAG does not match Cargo.toml version $CARGO_VERSION (bump the version, commit, then tag)"; exit 1
    fi
    ;;
esac
if grep -q '^name = "aws-lc-sys"' Cargo.lock; then
  echo "FAIL: aws-lc-sys is in Cargo.lock; keep reqwest on rustls with ring so every target links"; exit 1
fi
TOOLCHAIN="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
echo "rust-toolchain.toml: $TOOLCHAIN"
echo "check-version ok"
