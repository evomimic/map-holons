#!/bin/sh

set -eu

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

git config core.hooksPath .githooks

echo "Configured Git hooks path:"
echo "  core.hooksPath=.githooks"
echo
echo "Git will now run the tracked pre-push hook from .githooks/pre-push"
