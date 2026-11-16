#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
generated_import="${repository_root}/generated/json-imports/agent-spaces/schema.json"
packaged_package="${repository_root}/host/conductora/resources/schema-packages/agent-spaces"

mkdir -p "$packaged_package/imports"
cp "$generated_import" "$packaged_package/imports/schema.json"
