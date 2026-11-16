#!/usr/bin/env bash
set -euo pipefail

source_dir="generated/visualizer-commons/default-presentation"
target_dir="host/conductora/resources/visualizer-commons/default-presentation"

mkdir -p "$target_dir/imports"
cp "$source_dir/imports/schema.json" "$target_dir/imports/schema.json"
