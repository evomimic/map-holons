#!/usr/bin/env bash
# Own each run's temporary data independently of the surrounding Nix shell.
set -uo pipefail

run_root="${MAP_TEST_RUN_ROOT:-${HOME}/.cache/map-holons/test-runs}"
mkdir -p "$run_root" || exit 1
run_dir=$(mktemp -d "$run_root/run-XXXXXXXX") || exit 1
mkdir "$run_dir/tmp" || exit 1
export TMPDIR="$run_dir/tmp"

finish() {
  run_status=$?
  trap - EXIT
  printf '%s\n' "$run_status" > "$run_dir/exit-status"
  du -sh "$TMPDIR" > "$run_dir/temp-usage.txt"
  printf '\n[test-run] Temporary storage: '
  cat "$run_dir/temp-usage.txt"
  if [[ "$run_status" -eq 0 && "${MAP_TEST_KEEP_TEMP:-0}" != 1 ]]; then
    rm -rf -- "$TMPDIR"
    printf '[test-run] Temporary data removed; log: %s/test.log\n' "$run_dir"
  else
    printf '[test-run] Temporary data and log retained: %s\n' "$run_dir"
  fi
  exit "$run_status"
}
trap finish EXIT
# Interrupted runs retain their files so cleanup cannot race a surviving child.
trap 'exit 130' INT
trap 'exit 143' TERM

printf '[test-run] Run directory: %s\n' "$run_dir"
if [[ "$#" -eq 0 ]]; then
  set -- npm run test:run
fi
"$@" 2>&1 | tee "$run_dir/test.log"
pipeline_status=("${PIPESTATUS[@]}")
if [[ "${pipeline_status[0]}" -ne 0 ]]; then
  exit "${pipeline_status[0]}"
fi
exit "${pipeline_status[1]}"
