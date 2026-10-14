#!/usr/bin/env bash
set -euo pipefail

# Build once; each measurement then creates its own isolated Sweettest conductor.
npm run build:happ
python3 scripts/measure-commit-performance.py "$@"
