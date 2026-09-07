# Core Schema Bootstrap Performance Measurement

Issue #688 measures the generated Core Schema bootstrap graph before selecting any optimization.

Run one real-bundle measurement:

```sh
npm run sweet:bootstrap-perf
```

The command builds the normal hApp artifacts, creates one fresh Sweettest runtime, and loads the same generated Core Schema bootstrap bundle that ordinary Sweettests use. Capture the aggregate `[PERF-688]` records only; they avoid per-holon or per-link logging that would distort the result.

Run at least five times for each condition and report median, minimum, and maximum:

- **Warm:** repeat without clearing conductor/DNA caches.
- **Cold:** use an explicitly fresh conductor/DNA cache according to the local Sweettest procedure.

The logs separately report host parsing and dance round-trip time; loader phases; holon persistence; SmartLink expansion/action creation; semantic forward/inverse actions; keyed `Owns` index checks; lineage target materialization; inverse dedup expansion; and exact historical reads. Preserve the current Core Schema bundle and identical logging configuration across compared runs.

Do not interpret a timing reduction as permission to change ownership, keyed discovery, immutable history, or native Delete semantics. Any optimization must be justified by these counts and timings and reviewed separately.

## Inverse-dedup before baseline

Captured 2026-09-07 from three fresh focused-test processes, with `RUST_LOG=info` and
`WASM_LOG=info`. This is the comparison baseline for commit-local inverse membership indexing;
retain the same fixture, binary configuration, and log filter for the after run.

| Metric | Median | Range |
| --- | ---: | ---: |
| Focused test | 30.00s | 29.98–30.07s |
| Loader-client total | 26.57s | 26.51–26.58s |
| Sweettest conductor call | 26.29s | 26.24–26.30s |
| Guest commit | 8.62s | 8.57–8.63s |
| Inverse-dedup expansion and scan | 1.031s | 1.031–1.034s |
| Inverse-dedup expansions | 495 | 495–495 |
| Inverse-dedup candidates | 122,265 | 122,265–122,265 |
| Inverse-dedup skips | 0 | 0–0 |

The inverse-dedup elapsed value is nested within guest commit. It must not be subtracted or
added to other nested spans when calculating end-to-end savings.

## Inverse-dedup after sample

Captured 2026-09-07 from one fresh focused-test process after the commit-local inverse-membership
index was introduced. Repeat this measurement to the same five-run standard before treating the
timing values as a median comparison.

| Metric | Observed value |
| --- | ---: |
| Focused test | 28.85s |
| Loader-client total | 25.46s |
| Sweettest conductor call | 25.19s |
| Guest commit | 7.55s |
| Inverse-dedup bucket-load time | <1ms |
| Inverse-dedup expansions | 1 |
| Inverse-dedup candidates | 0 |
| Inverse-dedup membership checks | 495 |
| Inverse-dedup skips | 0 |

This confirms the algorithmic result. The Core Schema fixture starts with no existing `Owns`
memberships in its one inverse bucket, so the one required seed expansion returns no candidates.
The context still seeds from persisted occurrence-free local links for populated buckets, and adds
members only after a successful inverse write.
