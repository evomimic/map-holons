#!/usr/bin/env bash
# Run high-risk regressions before the rest of the long conductor suite.
set -euo pipefail

nextest_profile="${MAP_NEXTEST_PROFILE:-ci}"
nextest_args=(run --manifest-path tests/sweetests/Cargo.toml -p holons_test --profile "$nextest_profile" --no-capture)

if [[ "$nextest_profile" == ci-rest ]]; then
  # Keep this selection within ci-rest's default filter. The second invocation
  # excludes exactly these tests, so successful runs retain full coverage once.
  regression_filter='(binary(commit_incomplete_tests) & test(=outstanding_graph_replay_rejects_removal_then_composes_live_addition)) | binary(effective_descriptor_tests) | binary(relationship_cache_policy_tests)'
  printf '[sweettest] Running priority regressions (stop on first failure).\n'
  cargo nextest "${nextest_args[@]}" --fail-fast -E "$regression_filter"
  printf '[sweettest] Priority regressions passed; running remaining tests.\n'
  cargo nextest "${nextest_args[@]}" -E "not ($regression_filter)"
else
  cargo nextest "${nextest_args[@]}"
fi
