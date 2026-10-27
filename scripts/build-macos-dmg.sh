#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 /path/to/Eavesdrop.app /path/to/output.dmg" >&2
  exit 1
fi

app_path=$1
disk_image=$2
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(dirname "$script_dir")

if [ ! -d "$app_path/Contents" ]; then
  echo "Application bundle not found: $app_path" >&2
  exit 1
fi

staging_dir=$(mktemp -d "${TMPDIR:-/tmp}/eavesdrop-dmg.XXXXXX")
trap 'rm -rf "$staging_dir"' EXIT
trap 'exit 1' HUP INT TERM

# Preserve the signed/stapled bundle before the DMG builder copies it.
ditto "$app_path" "$staging_dir/$(basename "$app_path")"

# Write Finder metadata directly, so the same layout works in CI without
# launching Finder or granting automation permissions. Keep dependencies local
# to this build, rather than modifying the user's Python installation.
python3 -m venv "$staging_dir/tools"
"$staging_dir/tools/bin/python" -m pip install --disable-pip-version-check \
  --quiet -r "$script_dir/dmg-requirements.txt"

mkdir -p "$(dirname "$disk_image")"
"$staging_dir/tools/bin/dmgbuild" \
  -s "$script_dir/dmg-settings.py" \
  -D "app=$staging_dir/$(basename "$app_path")" \
  -D "background=$project_dir/assets/dmg-background.tiff" \
  Eavesdrop "$disk_image"
