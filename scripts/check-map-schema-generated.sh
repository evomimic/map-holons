#!/usr/bin/env bash
set -euo pipefail

scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

npm run map-schema:check:coreschema
npm run map-schema:compile -- schema-src --out-dir "$scratch_dir/imports"

if ! diff -ru \
  --exclude '.DS_Store' \
  --exclude 'core-schema-bootstrap.json' \
  generated/json-imports "$scratch_dir/imports"; then
  echo "Generated schema imports are stale. Run: npm run map-schema:compile:coreschema" >&2
  exit 1
fi

npm run map-schema -- roundtrip-json generated/json-imports \
  --tdl-out "$scratch_dir/tdl" \
  --json-out "$scratch_dir/roundtrip-json"
