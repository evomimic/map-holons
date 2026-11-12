#!/usr/bin/env bash
set -euo pipefail

# Opt-in diagnostics add no coordinator exports or schema fields. Keep the diagnostic
# artifact local to this run and restore ordinary production artifacts even on failure.
restore_artifacts() {
  local test_status=$?
  trap - EXIT
  if ! npm run build:happ; then
    exit 1
  fi
  exit "$test_status"
}
trap restore_artifacts EXIT
(
  cd happ
  CARGO_TARGET_DIR=target cargo build -p holons_integrity -p holons --release \
    --target wasm32-unknown-unknown --features holons/commit-audit
  hc app pack workdir --recursive
)
npm run build:probes -w map-happ
npm run check:happ-artifacts
cargo test --manifest-path tests/sweetests/Cargo.toml \
  --test commit_scalar_constraints_tests -- --ignored --nocapture
