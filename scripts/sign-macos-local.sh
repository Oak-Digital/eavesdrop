#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(dirname "$script_dir")
app_path=${1:-"$project_dir/src-tauri/target/release/bundle/macos/Eavesdrop.app"}

codesign \
  --force \
  --deep \
  --options runtime \
  --sign - \
  --identifier com.eavesdrop.recorder \
  --requirements '=designated => identifier "com.eavesdrop.recorder"' \
  --entitlements "$project_dir/src-tauri/Entitlements.plist" \
  "$app_path"

codesign --verify --deep --strict --verbose=2 "$app_path"
