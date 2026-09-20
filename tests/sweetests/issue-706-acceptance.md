# Issue 706 acceptance evidence

This records functional acceptance against [Issue 706](https://github.com/evomimic/map-holons/issues/706) and its [closeout plan](https://github.com/evomimic/map-holons/issues/706#issuecomment-5749850030). It is a test record, not an architectural specification. The authoritative design remains the Space Navigator implementation plan in `map-dev-docs`.

## Acceptance mapping

| Criterion | Implementation and evidence |
| --- | --- |
| Rust selects Properties, Property, and Value | `host/crates/dahn_selection/src/selection.rs`; `tests/property_selection.rs` covers each role, direct/inherited applicability, no candidate, ambiguity, and missing/multiple declared ValueType relationships. Value selection starts at the property's declared ValueType rather than its raw value. |
| Descriptor-backed SDK context and request projection | `host/map-sdk/tests/sdk/descriptors.test.ts` and `visualizer-selection.test.ts` cover declared metadata, cardinality failures, subject/parent request projection, selected-response binding, and propagation of Rust selection failures. |
| Properties below Node actions and above collections | `holon-inspector.artifact.test.ts` plus `property-presentation.acceptance.test.ts`; browser inspection of the committed Book artifacts confirms the left content column and ordering. |
| Parent-owned nested composition | `properties.js` supplies named Property wrappers; `property.js` mounts the supplied Value child. Artifact tests verify nested ownership and the three existing theme-controlled slot-border tokens. |
| Five scalar families, read-only, absent optional values | Book schema defines Title, IsPublished, PageCount, PublicationStatus, CoverDigest, and Subtitle. The existing Book/Person fixture commits String, Boolean false, Integer zero, Enum Draft, and three Bytes; Subtitle remains absent. Scalar artifact tests also cover negative integers, empty Bytes, literal HTML text, and unsupported input containment. |
| No TypeScript semantic fallback or stale constructor reuse | Materialization failures propagate; region boundaries preserve diagnostics. Registration returns a constructor-specific tag when the role tag already belongs to another implementation. Identical materialized source shares one imported constructor; failed imports permit retry. |
| DAHN relocation | `DahnMaterializer` and `DancerPackageCatalog` remain under `holons_client::dahn`; their unit tests pass with the focused host suite. |
| Table behavior preserved | Existing table, canvas, registry, adapter, and boundary tests pass with the UI suite. No table-cell integration is added. |
| Generated resources consistent | TDL compilation, bootstrap regeneration, schema check and round-trip fidelity pass; all authored executable digests match their bundled JS files. SDK fixture regeneration produces no diff. |

## Validation recorded on 2026-09-20

- `npm run check` in Nix: host, hApp WASM, and UI typecheck passed.
- Focused host suite (`dahn_selection`, `holons_client`, `map_commands_runtime`, `map_commands_wire`): 54 tests passed, including the relocated DAHN tests. The fixture generator is intentionally ignored in this suite and was run separately.
- SDK: 270 tests passed; SDK typecheck passed.
- UI with committed acceptance evidence: 62 tests passed across 22 files.
- Focused Holochain `book_value_presentation_acceptance`: passed. This executes the loader/commit path, descriptor discovery, command selection with parent slot checks, verified materialization, and one-use artifact retrieval.
- Existing Holochain `runtime_behavior_matrix`: passed (261 seconds), including the extended fixture and downstream runtime scenarios.
- Schema compiler: 42 tests passed; generated source check and 21-file round-trip passed.
- SDK fixture regeneration and repository formatting checks passed.
- Browser inspection of the exact exported artifacts confirmed scalar values, absent Subtitle, nested borders, and Properties/Collections placement.
- The user reported `npm test` and `npm start` nominal at checkpoint `ad177ed1`, before the closeout edits.

The exported-artifact browser check is a focused rendering harness, not an automated interaction with the running Conductora application. Schema-load performance is explicitly outside this closeout. Existing startup/cache/performance work is retained in the earlier checkpoint commit; it was not expanded as part of this closeout.

## Reproduce the focused acceptance path

Run from the repository root inside `nix develop`:

```sh
MAP_VALUE_PRESENTATION_EVIDENCE=/tmp/706-value-presentation.json \
  cargo test --manifest-path tests/sweetests/Cargo.toml \
  --test dance_tests book_value_presentation_acceptance -- --ignored --nocapture
MAP_VALUE_PRESENTATION_EVIDENCE=/tmp/706-value-presentation.json npm run web:test
node scripts/render-value-presentation-acceptance.mjs \
  /tmp/706-value-presentation.json /tmp/706-value-presentation.html
```

The JSON contains only fixture values and the verified Rust-selected module sources. The browser harness consumes those selections without discovering candidates or substituting a fallback. Without the evidence environment variable, the ordinary UI suite skips only the committed-artifact acceptance test; the scalar and composition artifact tests still run.

The focused Rust test is ignored in ordinary runs to avoid duplicating bootstrap; its assertions also run in `runtime_behavior_matrix`. The existing `runtime_behavior_matrix` Sweettest exercises other consumers of the extended Book/Person schema. GitHub CI and final merge status must be evaluated on the delivered PR head; local acceptance alone does not establish those results.
