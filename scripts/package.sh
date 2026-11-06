#!/usr/bin/env bash
# Builds Cleaner.app as a universal (arm64 + x86_64) bundle, signs it ad hoc,
# zips it for a GitHub release and renders the Homebrew cask for that zip.
#
#   scripts/package.sh            -> dist/Cleaner.app, dist/Cleaner-<v>-macos.zip, dist/cleaner.rb
#
# Ad hoc signing needs no Apple certificate. Apple Silicon refuses to run
# unsigned code, and a bundle needs one signature over all its parts.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
APP="$DIST/Cleaner.app"
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
BINARIES=(cleaner-ui cleaner)

die() {
  echo "[-] $*" >&2
  exit 1
}

version() {
  awk -F'"' '/^version *=/ {print $2; exit}' "$ROOT/Cargo.toml"
}

build() {
  local t
  for t in "${TARGETS[@]}"; do
    echo "[*] building $t"
    cargo build --release --workspace --target "$t" --manifest-path "$ROOT/Cargo.toml"
  done
}

# The window starts the engine from its own folder, so both live in MacOS/.
bundle() {
  local v="$1" b
  rm -rf "$APP"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  for b in "${BINARIES[@]}"; do
    lipo -create -output "$APP/Contents/MacOS/$b" \
      "$ROOT/target/aarch64-apple-darwin/release/$b" \
      "$ROOT/target/x86_64-apple-darwin/release/$b"
  done
  sed "s/@VERSION@/$v/g" "$ROOT/packaging/Info.plist" >"$APP/Contents/Info.plist"
  codesign --force --deep --sign - "$APP"
  codesign --verify --deep --strict "$APP"
}

main() {
  command -v lipo >/dev/null || die "lipo not found (install Xcode command line tools)"
  local v zip sum
  v="$(version)"
  [[ -n "$v" ]] || die "no version in Cargo.toml"

  build
  bundle "$v"

  zip="$DIST/Cleaner-$v-macos.zip"
  rm -f "$zip"
  ditto -c -k --keepParent "$APP" "$zip"
  sum="$(shasum -a 256 "$zip" | awk '{print $1}')"
  sed -e "s/@VERSION@/$v/g" -e "s/@SHA256@/$sum/g" "$ROOT/packaging/cleaner.rb.in" >"$DIST/cleaner.rb"

  echo "[+] $APP"
  echo "[+] $zip"
  echo "[+] $DIST/cleaner.rb (sha256 $sum)"
}

main "$@"
