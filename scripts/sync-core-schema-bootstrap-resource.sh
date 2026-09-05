#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
generated_bundle="${repository_root}/generated/core-schema-bootstrap"
packaged_bundle="${repository_root}/host/conductora/resources/core-schema-bootstrap"

if [[ ! -f "${generated_bundle}/manifest.json" ]]; then
  echo "Generated Core Schema bootstrap bundle is missing: ${generated_bundle}" >&2
  exit 1
fi

mkdir -p "${packaged_bundle}"
cp -R "${generated_bundle}/." "${packaged_bundle}/"
