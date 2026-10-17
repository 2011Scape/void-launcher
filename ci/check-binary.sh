#!/usr/bin/env bash
# Sanity checks so a bad build fails in CI rather than on a user's machine.
set -euo pipefail
KIND="${1:?linux|windows|macos}"; BIN="${2:?binary}"
case "$KIND" in
  linux)
    # The Linux build runs on the oldest Ubuntu GitHub still offers; report the glibc it needs.
    MAX="$(objdump -T "$BIN" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1 | sed 's/GLIBC_//')"
    echo "highest glibc symbol: $MAX"
    file "$BIN" | grep -q 'ELF 64-bit' || { echo "FAIL: not a 64-bit ELF"; exit 1; }
    ;;
  windows)
    file "$BIN"
    file "$BIN" | grep -qiE 'PE32\+.*(x86-64|GUI)' || { echo "FAIL: not a 64-bit PE"; exit 1; }
    # A GUI subsystem keeps a console window from flashing up behind the launcher.
    if command -v objdump >/dev/null; then
      SUBSYS="$(objdump -p "$BIN" | awk '/^Subsystem/{print $2}')"
      [ "$SUBSYS" = "00000002" ] || { echo "FAIL: subsystem $SUBSYS is not GUI (2)"; exit 1; }
    fi
    ;;
  macos)
    file "$BIN"
    file "$BIN" | grep -qE 'Mach-O' || { echo "FAIL: not a Mach-O binary"; exit 1; }
    ;;
esac
echo "check-binary: $KIND ok"
