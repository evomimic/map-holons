#!/usr/bin/env bash
# Launch the packaged local constitution workflow against its persistent store.
set -euo pipefail
host_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
application="$host_dir/target/debug/bundle/macos/Map Holons.app/Contents/MacOS/MAP"
if [[ ! -x "$application" ]]; then
  echo 'Build Welcome first: npm run welcome:build -w map-host' >&2
  exit 1
fi
exec env -u MAP_START_MODE -u STORAGE_CONFIG_PATH -u MAP_APPLICATION_EXPERIENCE "$application" "$@"
