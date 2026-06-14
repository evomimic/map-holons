#!/usr/bin/env bash

set -euo pipefail

saved_tty_state=""

if [[ -t 0 ]]; then
  saved_tty_state="$(stty -g)"
fi

restore_tty() {
  if [[ -n "${saved_tty_state}" ]] && [[ -t 0 ]]; then
    stty "${saved_tty_state}" || stty sane || true
  fi
}

trap restore_tty EXIT INT TERM HUP

tauri dev "$@"
