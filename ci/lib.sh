#!/usr/bin/env bash
# Shared by every CI job and the justfile: resolves the build identity once.
#   tag  v1.2.0        -> VERSION=1.2.0            CHANNEL=stable
#   push to main / PR  -> VERSION=<cargo>-dev.<sha> CHANNEL=development
#   local              -> VERSION=<cargo>-local     CHANNEL=development
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
FULL_SHA="${GITHUB_SHA:-$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)}"
SHORT_SHA="${FULL_SHA:0:8}"

if [[ "${GITHUB_REF:-}" == refs/tags/v* ]]; then
  VERSION="${GITHUB_REF#refs/tags/v}"
  CHANNEL="stable"
elif [ -n "${GITHUB_SHA:-}" ]; then
  VERSION="${CARGO_VERSION}-dev.${SHORT_SHA}"
  CHANNEL="development"
else
  VERSION="${CARGO_VERSION}-local"
  CHANNEL="development"
fi
COMMIT="$FULL_SHA"
BUILT_AT="${BUILT_AT:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"

export VERSION CHANNEL COMMIT BUILT_AT CARGO_VERSION SHORT_SHA ROOT
export VOID_VERSION="$VERSION" VOID_CHANNEL="$CHANNEL" VOID_COMMIT="$SHORT_SHA" VOID_BUILT_AT="$BUILT_AT"
echo "version=$VERSION channel=$CHANNEL commit=$SHORT_SHA built_at=$BUILT_AT"
