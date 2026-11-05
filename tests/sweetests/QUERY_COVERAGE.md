# Query test coverage inventory

This records the executable coverage allocation in this checkout. Architectural authority remains
[the MAP testing strategy](../../../map-dev-docs/docs/core/map-holons-testing-strategy.md) and
[the query design](../../../map-dev-docs/docs/core/map-queries/query-engine-design-spec.md).

## Default checks

- `holons_core` query-layer unit tests own the semantic matrix. Their in-memory subjects use
  production descriptor navigation for saved application reads, including missing declarations.
- `fixture_authoring_tests` checks fixture construction and finalization without a conductor.
- `dance_tests::query_runtime` boots one real runtime, runs the saved unsupported-expression
  scaffold and one shared dataset fixture, then checks storage and adapter contracts.
- `query_semantic_matrix` and `query_fixture::*` retain the full live matrix for explicit diagnosis.
  They are ignored by normal runs and are not a substitute for the default unit/contract checks.

## Coverage owners

Unit paths below are under `shared_crates/holons_core/src/`. Contract cases are in
`tests/query_contract/cases.rs`; fixture construction is in `tests/fixture_cases/query_boundary_fixture.rs`.

| Requirement | Fast semantic owner | Real-storage / adapter contract |
|---|---|---|
| Saved Query / expression lookup and unsupported kind | `query_layer/query_core.rs::unsupported_expression_still_not_implemented` | QRY1 saved scaffold, supplied and absent input, Direct and QueryDance |
| Declared empty versus undeclared navigation | `query_layer/query_core.rs::saved_reads_distinguish_undeclared_from_empty_relationships` | Saved unsupported scaffold; saved declared expansion includes a source with no members |
| Inherited declarations and duplicate declarations | `descriptors/holon_descriptor.rs::flattened_plural_accessors_preserve_self_first_inheritance_order`, `get_relationship_by_name_detects_duplicate_inherited_declarations` | Actual loaded Book/Person descriptors and real inverse navigation |
| Owns membership, order and duplicates | `query_layer/query_core.rs::seed_holons_root_expands_owns_in_order_with_duplicates` | One unfiltered saved SeedHolons execution; fixture roots must be present |
| Seed input contract | `query_layer/query_core.rs::seed_holons_rejects_supplied_input` | QueryDance forwards supplied input to a saved SeedHolons and receives InvalidParameter |
| SeedPredicate | `query_layer/query_core.rs::a_seed_predicate_is_validated_against_the_owns_target_type`, `nested_attachments_are_validated_against_their_declared_types` | One filtered SeedHolons through QueryDance, selecting by Key using actual root/value/operator descriptors |
| Declared / inverse Expand and source ordering | `query_layer/query_core.rs::expand_declared_name_preserves_source_and_target_order`, `expand_inverse_name_resolves_through_descriptor` | Saved queries over AuthoredBy and AuthorOf; exact result IDs, order and duplicates |
| Unknown relationship errors | `query_layer/query_core.rs::expand_unknown_name_propagates_descriptor_error` | QueryDance propagates DescriptorDeclarationNotFound from real Book schema |
| Single-holon convenience | `query_layer/query_core.rs::single_holon_convenience_wraps_a_transient_singleton_collection` | Saved Expand with one Book input and exact author results |
| Contains / Equals domains, absence, representation and scalar variants | `query_layer/predicate.rs` comparison matrix | Actual string Contains and enum Equals in the composed pipeline |
| Not / AllOf / AnyOf truth tables, sharing and cycles | `query_layer/predicate.rs` recursive predicate matrix | Production authoring helpers create one nested predicate over real Book descriptors |
| Filter and Distinct ordering and composition | `query_layer/query_core.rs::filter_and_distinct_compose_in_either_order`, `a_nested_filter_composes_with_distinct_in_either_order` | Expand → nested Filter → Distinct → OrderBy → Skip → Limit, exact final ID |
| Sort domains, defaults, ties, null placement and invalid specs | `query_layer/order_by.rs`, `query_layer/query_core.rs` ordering matrix | Integer PageCount with descriptor defaults and a missing value in the pipeline |
| Pagination counts and operator order | `query_layer/query_core.rs::authored_order_decides_the_page`, `pagination_count_boundaries`, `invalid_counts_fail_even_on_empty_input` | Nonzero Skip and Limit in the pipeline; removing either changes the expected result |
| Execution record lifecycle and chain identity | `query_layer/query_core.rs` chain success/failure tests | Shared direct executor checks Pending/Complete/Failed, element types and predecessor-result identity |
| Adapter supplied input and response wrapping | QueryCore units own evaluation | A Limit query receives an ordered input with duplicates; response-body type and exact IDs are checked |
| Adapter absent input | QueryCore units own source behavior | Filtered SeedHolons succeeds without InitialInput |
| Adapter binding and error forwarding | `query_layer/query_core.rs::nonempty_bindings_are_refused_before_any_validation_or_record` | Nonempty RequestParameters produces NotImplemented; unknown navigation error propagates |
| Adapter malformed requests | Descriptor/Dance contract validation | Missing Request, missing RequestedQuery and wrong InitialInput kind are rejected |

The composed pipeline uses five Books and two People. Its expected result is authored explicitly.
Changing Filter, Distinct, ordering, Skip or Limit changes that result; operator-by-operator edge
conditions remain independently covered by the unit matrix. Query definitions use production
reference/authoring APIs and remain transient. Stored subjects and saved definitions go through
normal schema loading, staging and Commit; setup does not recompare their complete graphs.

The compact suite has two logical Owns expansions. Cache policies determine physical storage
reads. Predicate truth-table variants use small in-memory subjects rather than repeatedly filtering
the bootstrap space.

## Running

Inside Nix, with fresh hApp artifacts when guest code changed:

```sh
# Fast semantics; no conductor.
cargo test --manifest-path shared_crates/holons_core/Cargo.toml --lib query_layer::

# Default real-storage / adapter contract.
RUST_LOG=warn WASM_LOG=warn cargo test --manifest-path tests/sweetests/Cargo.toml \
  --test dance_tests query_runtime -- --exact

# Explicit full live semantic matrix.
RUST_LOG=warn WASM_LOG=warn cargo test --manifest-path tests/sweetests/Cargo.toml \
  --test dance_tests query_semantic_matrix -- --exact --ignored

# One full fixture after a targeted failure.
RUST_LOG=warn WASM_LOG=warn cargo test --manifest-path tests/sweetests/Cargo.toml \
  --test dance_tests query_fixture::case_6_boolean -- --exact --ignored
```

The query result assertions compare exact identities, order and multiplicity.
Definition immutability scans and blanket per-query database scans are not part of the contract tests.

## Local verification — 2026-11-05

The compact `query_runtime` passed in **254.04 seconds** (4m14s), including fresh
conductor/Core Schema bootstrap, versus **1,019.10 seconds** (16m59s) for the preceding
full query group on this Mac: a **75.1% reduction** in this pair of runs. Compilation is
excluded. This is a local comparison, not a cross-machine runtime guarantee.

- Declarative fixture steps: **1,621 → 74**.
- Query invocations: **187 → 14**, plus three adapter contract rejections.
- Logical SeedHolons/Owns expansions: **8 → 2**.
- All **692 core unit tests** passed, including **176 query-layer tests**.
- All **3 fixture-authoring tests**, Rust formatting and the hApp WASM check passed.

The complete live semantic matrix was retained but was not rerun after this test allocation
change; it is an explicit diagnostic selector. Its semantic variants remain in the default
unit tests, while `query_runtime` owns the selected real-storage and adapter boundaries.
