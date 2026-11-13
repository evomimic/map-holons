#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
generated_imports="${repository_root}/generated/house-troupe/space-navigator/imports"
packaged_package="${repository_root}/host/conductora/resources/house-troupe/space-navigator"

if [[ ! -f "${generated_imports}/schema.json" ]]; then
  echo "Generated Space Navigator schema import is missing: ${generated_imports}/schema.json" >&2
  exit 1
fi

mkdir -p "${packaged_package}/imports"
cp "${generated_imports}/schema.json" "${packaged_package}/imports/schema.json"
