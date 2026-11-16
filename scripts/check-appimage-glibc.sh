#!/usr/bin/env bash
# Fail if the built AppImage needs a newer glibc than the given baseline.
#
# AppImages bundle most libraries (WebKitGTK, GLib, ...) but deliberately use
# the host's glibc, so the newest GLIBC_x.y symbol referenced by anything in
# the AppDir is the minimum glibc a user's distro must have. This catches
# the build runner silently moving to a newer Ubuntu.
#
# Usage: scripts/check-appimage-glibc.sh <max-glibc-version> [AppImage]
#   e.g. scripts/check-appimage-glibc.sh 2.35

set -euo pipefail

MAX="${1:?usage: $0 <max-glibc-version> [AppImage]}"
APPIMAGE="${2:-}"

if [ -z "$APPIMAGE" ]; then
  ROOT="${GITHUB_WORKSPACE:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
  APPIMAGE=$(find "$ROOT/target" "$ROOT/crates/libretune-app/src-tauri/target" \
    -path '*/bundle/appimage/*.AppImage' -type f 2>/dev/null | head -n1 || true)
fi
if [ -z "$APPIMAGE" ] || [ ! -f "$APPIMAGE" ]; then
  echo "Error: no AppImage found" >&2
  exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
APPIMAGE=$(realpath "$APPIMAGE")
chmod +x "$APPIMAGE"
(cd "$WORK" && "$APPIMAGE" --appimage-extract >/dev/null)

# Collect the newest GLIBC_ version referenced by each ELF file.
report=$(
  find "$WORK/squashfs-root" -type f | while read -r f; do
    head -c4 "$f" 2>/dev/null | grep -q $'^\x7fELF' || continue
    v=$(objdump -T "$f" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' \
      | sed 's/GLIBC_//' | sort -uV | tail -n1)
    if [ -n "$v" ]; then echo "$v ${f#"$WORK/squashfs-root/"}"; fi
  done | sort -V
)

newest=$(echo "$report" | tail -n1 | cut -d' ' -f1)
echo "Newest glibc required by $(basename "$APPIMAGE"): ${newest:-none} (allowed: <= $MAX)"

if [ -n "$newest" ] && [ "$(printf '%s\n%s\n' "$MAX" "$newest" | sort -V | tail -n1)" != "$MAX" ]; then
  echo "Error: AppImage requires glibc newer than $MAX. Offending files:" >&2
  echo "$report" | while read -r v f; do
    if [ "$(printf '%s\n%s\n' "$MAX" "$v" | sort -V | tail -n1)" != "$MAX" ]; then
      echo "  GLIBC_$v  $f" >&2
    fi
  done
  echo "Build on an older runner (see runs-on for build-linux)." >&2
  exit 1
fi
