#!/usr/bin/env bash
# Compares the Go and Rust builds of cleaner on binary size, scan speed,
# delete speed and peak memory. Usage: scripts/compare.sh GO_BIN RUST_BIN
#
# The scan runs read only (--dry-run) over the real home directory. The
# delete benchmark only ever touches a generated tree under BENCH_DIR, and
# passes --roots and --only so no other rule or folder is in reach.
set -euo pipefail

RUNS="${RUNS:-5}"
PROJECTS="${PROJECTS:-8}"
PACKAGES="${PACKAGES:-400}"
FILES="${FILES:-25}"
BENCH_DIR="$HOME/.cache/cleaner-bench"

die() {
  echo "[-] $*" >&2
  exit 1
}

# Prints "wall_seconds max_rss_bytes" for one run of the given command.
measure() {
  local out
  out=$( { /usr/bin/time -l "$@" >/dev/null; } 2>&1 )
  local wall rss
  wall=$(awk '/ real /{print $1}' <<<"$out")
  rss=$(awk '/maximum resident set size/{print $1}' <<<"$out")
  echo "$wall $rss"
}

median() {
  sort -n | awk '{a[NR]=$1} END {print (NR%2) ? a[(NR+1)/2] : (a[NR/2]+a[NR/2+1])/2}'
}

make_tree() {
  rm -rf "$BENCH_DIR"
  local p
  for ((p = 1; p <= PROJECTS; p++)); do
    local proj="$BENCH_DIR/proj$p"
    mkdir -p "$proj"
    echo '{}' >"$proj/package.json"
    (
      cd "$proj"
      eval "mkdir -p node_modules/pkg{1..$PACKAGES}/lib/src"
      eval "touch node_modules/pkg{1..$PACKAGES}/lib/src/f{1..$FILES}.js"
    )
  done
}

# Runs one benchmark RUNS times after a warmup and prints
# "median_wall median_rss_mib".
bench() {
  local label="$1" setup="$2"
  shift 2
  local walls=() rsses=() i
  [[ -n "$setup" ]] && $setup
  "$@" >/dev/null 2>&1 || true
  for ((i = 0; i < RUNS; i++)); do
    [[ -n "$setup" ]] && $setup
    read -r wall rss < <(measure "$@")
    walls+=("$wall")
    rsses+=("$rss")
  done
  local w r
  w=$(printf '%s\n' "${walls[@]}" | median)
  r=$(printf '%s\n' "${rsses[@]}" | median | awk '{printf "%.1f", $1 / 1048576}')
  echo "$w $r"
}

scan_paths() {
  "$1" --dry-run 2>/dev/null | grep -E '^ {20,}[0-9.]+ [KMGT]?i?B +[~/]' | sed -E 's/^ +[0-9.]+ [KMGT]?i?B +//' | sort
}

main() {
  [[ $# -eq 2 ]] || die "usage: $0 GO_BIN RUST_BIN"
  local go_bin="$1" rust_bin="$2"
  [[ -x "$go_bin" && -x "$rust_bin" ]] || die "both binaries must exist and be executable"
  trap 'rm -rf "$BENCH_DIR"' EXIT

  local files=$((PROJECTS * PACKAGES * FILES))
  echo "[*] runs=$RUNS, delete tree=$files files in $PROJECTS projects"

  echo "[*] checking both builds report the same paths"
  if diff <(scan_paths "$go_bin") <(scan_paths "$rust_bin") >/dev/null; then
    echo "[+] scan results match"
  else
    echo "[!] scan results differ:"
    diff <(scan_paths "$go_bin") <(scan_paths "$rust_bin") || true
  fi

  local delete_args=(--yes --only node-modules --roots "$BENCH_DIR")
  local go_scan rust_scan go_del rust_del
  echo "[*] scan benchmark"
  go_scan=$(bench scan "" "$go_bin" --dry-run)
  rust_scan=$(bench scan "" "$rust_bin" --dry-run)
  echo "[*] delete benchmark"
  go_del=$(bench delete make_tree "$go_bin" "${delete_args[@]}")
  rust_del=$(bench delete make_tree "$rust_bin" "${delete_args[@]}")

  local go_size rust_size
  go_size=$(stat -f %z "$go_bin")
  rust_size=$(stat -f %z "$rust_bin")

  echo
  printf '%-22s %14s %14s\n' "" "go" "rust"
  printf '%-22s %14s %14s\n' "binary size (KiB)" $((go_size / 1024)) $((rust_size / 1024))
  printf '%-22s %14s %14s\n' "scan wall (s)" "${go_scan% *}" "${rust_scan% *}"
  printf '%-22s %14s %14s\n' "scan peak RSS (MiB)" "${go_scan#* }" "${rust_scan#* }"
  printf '%-22s %14s %14s\n' "delete wall (s)" "${go_del% *}" "${rust_del% *}"
  printf '%-22s %14s %14s\n' "delete peak RSS (MiB)" "${go_del#* }" "${rust_del#* }"
}

main "$@"
