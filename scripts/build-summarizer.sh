#!/usr/bin/env bash
# Builds the summary engine and files it where Tauri's externalBin expects it:
# src-tauri/binaries/eavesdrop-summarizer-<target-triple>[.exe].
#
# This has to run before `tauri build`, which reads the binary rather than
# building it. Pass the same --target you pass to Tauri.
set -euo pipefail

target="${1:-}"
if [ -z "$target" ]; then
  echo "usage: $0 <target-triple>" >&2
  echo "  e.g. $0 universal-apple-darwin" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root/src-tauri"
out="binaries"
mkdir -p "$out"

build_one() {
  cargo build --release -p eavesdrop-summarizer --target "$1"
  echo "target/$1/release/eavesdrop-summarizer"
}

case "$target" in
  universal-apple-darwin)
    # A universal build compiles each arch separately before lipo'ing the app,
    # and each of those passes looks up the sidecar under its own triple. So
    # both per-arch names have to exist; the universal name is written too, for
    # whichever stage reaches for it.
    arm="$(build_one aarch64-apple-darwin)"
    intel="$(build_one x86_64-apple-darwin)"
    cp "$arm" "$out/eavesdrop-summarizer-aarch64-apple-darwin"
    cp "$intel" "$out/eavesdrop-summarizer-x86_64-apple-darwin"
    lipo -create "$arm" "$intel" -output "$out/eavesdrop-summarizer-$target"
    lipo -info "$out/eavesdrop-summarizer-$target"
    ;;
  *-pc-windows-*)
    built="$(build_one "$target")"
    cp "$built.exe" "$out/eavesdrop-summarizer-$target.exe"
    ;;
  *)
    built="$(build_one "$target")"
    cp "$built" "$out/eavesdrop-summarizer-$target"
    ;;
esac

echo "summary engine staged in src-tauri/$out"
