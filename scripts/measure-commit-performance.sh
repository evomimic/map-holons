#!/usr/bin/env bash
set -euo pipefail

# Build once; each measurement then creates its own isolated Sweettest conductor.
npm run build:commit-perf -w map-happ
python3 scripts/measure-commit-performance.py "$@"
