#!/usr/bin/env bash
# Unit tests for every crate under shared_crates/.
#
# shared_crates/ is deliberately not a Cargo workspace: its crates are path
# dependencies consumed separately by the host and hApp workspaces, so there is
# no --workspace to enumerate them. Discovering manifests from disk keeps this in
# step with new crates, which an explicit list does not -- a crate missing from a
# list is silently untested rather than failing.
set -uo pipefail

status=0

while IFS= read -r manifest; do
  echo "== $(dirname "$manifest")"
  # Report every crate's result rather than stopping at the first failure, so one
  # broken crate does not hide the state of the rest.
  cargo test --manifest-path "$manifest" || status=1
done < <(find shared_crates -name Cargo.toml -not -path '*/target/*' | sort)

if [ "$status" -ne 0 ]; then
  echo "One or more shared_crates unit test runs failed." >&2
fi

exit "$status"
