#!/bin/sh
set -eu

case "$(uname -s)" in
  Darwin)
    DATA_DIR="$HOME/Library/Application Support/com.map-holons.tauri.dev"
    ;;
  Linux)
    DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/com.map-holons.tauri.dev"
    ;;
  *)
    echo "clean:recovery: unsupported platform $(uname -s)" >&2
    exit 1
    ;;
esac

TARGET="$DATA_DIR/storage/local_recovery"
rm -rf "$TARGET"
echo "Recovery session data cleared: $TARGET"
