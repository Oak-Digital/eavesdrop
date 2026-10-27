#!/bin/sh
# Rebuild the macOS Dock / app-switcher icon from its glass master artwork.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(dirname "$script_dir")
source_icon="$project_dir/assets/icon-macos-glass.png"
output_icon="$project_dir/src-tauri/icons/icon.icns"

if [ "$(uname -s)" != Darwin ]; then
  echo "This script requires macOS sips and iconutil." >&2
  exit 1
fi
if [ ! -f "$source_icon" ]; then
  echo "Missing master artwork: $source_icon" >&2
  exit 1
fi

icon_workspace=$(mktemp -d "${TMPDIR:-/tmp}/eavesdrop-icon.XXXXXX")
trap 'rm -rf "$icon_workspace"' EXIT HUP INT TERM
iconset="$icon_workspace/Eavesdrop.iconset"
mkdir "$iconset"

for size in 16 32 128 256 512; do
  sips --resampleHeightWidth "$size" "$size" "$source_icon" \
    --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina_size=$((size * 2))
  sips --resampleHeightWidth "$retina_size" "$retina_size" "$source_icon" \
    --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil --convert icns "$iconset" --output "$output_icon"
echo "Updated $output_icon"
