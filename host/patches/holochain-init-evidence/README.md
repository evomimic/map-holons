# Initialization-backport validation evidence

## Matched authenticated reads

Captured on 2026-11-18 on one Mac using MAP commit
`9bf273a766dd79954b3c63be6ad0ec7e04b4e817`, Holochain 0.7.1, Rust 1.95.0,
and native `opt-level=z`. Both variants shared the same profiling-enabled guest
WASM, packed DNA/hApp, Core Schema fixtures, and diagnostic source. Each run
created a fresh disposable single-agent conductor/database and loaded 1,060
holons / 5,839 links. No compilation overlapped these measurements.

Order: control-1, patched-1, patched-2, control-2. Each run collected five exact
holon reads and five relationship reads through authenticated native
`AppWebsocket.call_zome`, with expected result/relationship identity assertions.
All four runs passed.

| Relationship interval (median) | Registry 0.7.1 | Backported 0.7.1 |
| --- | ---: | ---: |
| Complete authenticated call | 191.939 ms | 3.6525 ms |
| Internal adapter, including HDK reads | 2.8595 ms | 2.315 ms |
| Outside internal adapter timer | 187.2245 ms | 1.373 ms |

The residual was calculated per call before taking its median; independently
calculated medians need not subtract exactly. It includes combined pre/post
adapter work, including some guest entry/exit work; it is not a conductor-only
measurement. The two upstream changes were tested together.

Residual medians by run: **192.530 → 1.470 → 1.329 → 185.763 ms**.
Reduction across pooled samples: **185.8515 ms / 99.2667%**. Exact-holon complete
calls fell from 151.5395 ms to 2.6275 ms. Fresh-run wire sizes varied slightly;
both larger and smaller control payloads retained the large delay.

`matched-samples.json` contains every paired guest/client/logical sample;
`matched-summary.json` contains the pooled medians. The four `.log` files are
unmodified raw captures. `matched-provenance.json` retains the original source,
artifact and executable hashes and machine paths from the isolated experiment.
The original diagnostic sources and reproduction procedure are documented in
[MAP's performance findings](https://github.com/memetic-activation-platform/map-dev-docs/blob/939e253693e1acf0dd9215c81275f10316f31d8a/docs/core/performance/holochain-link-read-findings.md).
Machine paths in this evidence identify that capture, not paths required by this PR.

## Historical full npm test

After integrating the native backport into that same historical worktree's host
workspace, the complete standard `npm test` passed with exit status 0 in
**1,884.145 seconds (31m 24.145s)**. It included host compilation/tests, shared
and guest units, schema and fixture checks, web/SDK checks, production/probe
artifact audits, and the full standard Sweettest suite.

Sweettest reported **95 passed, 0 failed, 7 ignored** (including library/fixture
checks). `dance_tests` completed both the pristine loader and runtime matrix in
**584.51 seconds (9m 44.51s)**. Individual target results are in
`historical-sweettest-targets.json`; timing/configuration/artifact hashes are in
`historical-npm-test-summary.json`.

The successful attempt used warmed compilation caches and no concurrent user
npm test. Dependency installation was outside the timing. An earlier attempt
spent 10m 35.629s on cold compilation before stopping at two unused diagnostic
export inventory entries; another stopped at 1m 31.264s on the performance
helper's new query signature. Those setup issues were corrected before the
successful full run. The run preserved the checked-in optimization profiles:
host unoptimized, Sweettest `opt-level=z`.

This is an observed patched-suite duration, **not a controlled full-suite speedup
ratio** against the older multi-hour appliance run. It does not measure desktop
startup or Canvas navigation.

## Current-main adoption boundary

This PR is based on `dc70c6d7`, not the historical diagnostic commit. It preserves
main's already adopted DHT location-index/query and credential-wrapping fixes,
composes the source-chain backport with them, removes all active diagnostic
scaffolding, and retains only the required performance-helper API adaptation.
Neither guest profiling nor test conductor networking policy changes are part
of the adoption diff. Current-head validation is reported separately in the PR.

Current-main candidate checks completed locally: host workspace **324 passed / 0 failed / 1 ignored**; Sweettest library, fixture-authoring and persisted-query helper **71 passed / 0 failed / 1 ignored**; host and Sweettest formatting checks passed. Both lockfiles retain exactly main's dependency versions. See `current-main-checks.json`. Full `npm test` was not rerun on this current-main candidate. CI will exercise its configured checks; the 31m 24.145s result above is explicitly the historical integrated run.

The current-main production zomes built for `wasm32-unknown-unknown` in release mode, and the packed production DNA/hApp passed the current artifact audit with `--deny-production-test-only`.
