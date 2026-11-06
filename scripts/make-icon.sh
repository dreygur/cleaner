#!/usr/bin/env bash
# Builds packaging/AppIcon.icns from packaging/icon-source.jpeg.
#
#   scripts/make-icon.sh [crop]   crop = share of the artwork kept, default 0.73
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/packaging/icon-source.jpeg"
OUT="$ROOT/packaging/AppIcon.icns"
CROP="${1:-0.73}"
SIZES=(16 32 128 256 512)

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
set="$work/AppIcon.iconset"
mkdir "$set"

swift "$ROOT/packaging/make_icon.swift" "$SRC" "$work/icon-1024.png" "$CROP"

for s in "${SIZES[@]}"; do
  sips -z "$s" "$s" "$work/icon-1024.png" --out "$set/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$work/icon-1024.png" --out "$set/icon_${s}x${s}@2x.png" >/dev/null
done

iconutil -c icns "$set" -o "$OUT"
echo "[+] $OUT"
