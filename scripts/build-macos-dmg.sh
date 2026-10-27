#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 /path/to/Eavesdrop.app /path/to/output.dmg" >&2
  exit 1
fi

app_path=$1
disk_image=$2

if [ ! -d "$app_path/Contents" ]; then
  echo "Application bundle not found: $app_path" >&2
  exit 1
fi

staging_dir=$(mktemp -d "${TMPDIR:-/tmp}/eavesdrop-dmg.XXXXXX")
trap 'rm -rf "$staging_dir"' EXIT
trap 'exit 1' HUP INT TERM

# Preserve the signed/stapled bundle and put the install destination beside it.
ditto "$app_path" "$staging_dir/$(basename "$app_path")"
ln -s /Applications "$staging_dir/Applications"

mkdir -p "$(dirname "$disk_image")"
hdiutil create -volname Eavesdrop -srcfolder "$staging_dir" -ov -format UDZO "$disk_image"
