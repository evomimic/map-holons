# Dance Test Harness — Reference

This document covers harness internals. You only need this when:
- Writing a new `DanceTestStep` type (both adder + executor)
- Debugging unexpected resolution or recording behavior
- Understanding why commit does not return tokens

For test case authoring, the SKILL.md body is sufficient.

---

## 1. Two-Phase Separation

The harness enforces a strict boundary between phases:

**Fixture Phase** — intent declaration
- TestCase is constructed via `TestCaseInit::new()`
- Steps are added via `add_*_step()` adders
- `TestReference` tokens are minted and returned
- No execution, no conductor, no real holon state

**Execution Phase** — real dispatch
- Driven by `rstest_dance_tests` in `dance_tests.rs`
- Each `DanceTestStep` is matched and dispatched to its executor
- Executors resolve tokens → runtime `HolonReference`, execute the operation, validate, record
- Commands flow: `MapCommand` → `Runtime::execute_command()` → `TrustChannel` → SweetConductor → WASM guest → DHT

The harness is the **only** layer that connects these phases.

---

## 2. Core Abstractions

### TestReference — The Step Contract

```rust
pub struct TestReference {
    pub source: SourceSnapshot,   // what to operate on
    pub expected: ExpectedSnapshot, // what to expect after
}

pub struct SourceSnapshot {
    pub snapshot: TransientReference,  // identifies the fixture-time holon
    pub state: TestHolonState,         // intended lifecycle state at execution time
}

pub struct ExpectedSnapshot {
    pub snapshot: Option<TransientReference>,  // None iff state == Deleted
    pub state: TestHolonState,
}

pub enum TestHolonState { Transient, Staged, Saved, Abandoned, Deleted }
```

`TestReference` is **immutable and opaque** to test case authors. It flows from adder → step
enum variant → executor. It is never inspected by callers.

Snapshot identity is accessed via helper methods, not field access:
- `test_ref.source_snapshot_id() -> SnapshotId`
- `test_ref.expected_snapshot_id() -> Option<SnapshotId>`

### FixtureHolon — Logical Holon Identity Across Steps

```rust
pub struct FixtureHolon {
    pub id: FixtureHolonId,
    pub state: TestHolonState,
    pub head_snapshot: TransientReference,  // current authoritative snapshot
}
```

Tracks one logical entity as it moves through states. `head_snapshot` advances when a step
mutates the holon or when commit transitions it to Saved.

### FixtureHolons — Fixture-Time Registry (Sole Authority)

```rust
pub struct FixtureHolons {
    pub tokens: Vec<TestReference>,                              // append-only ledger
    pub holons: BTreeMap<FixtureHolonId, FixtureHolon>,         // logical identity registry
    pub snapshot_to_fixture_holon: BTreeMap<SnapshotId, FixtureHolonId>, // lookup index
}
```

`FixtureHolons` is the **only** component that may:
- Mint `TestReference`s
- Register snapshots
- Advance head snapshots
- Implement commit semantics

All snapshot registration and head advancement flows through `FixtureHolons` methods. No other
code may update `head_snapshot` or register snapshots.

### ExecutionHolons — Execution-Time Registry

```rust
pub struct ExecutionHolons {
    pub by_snapshot_id: BTreeMap<SnapshotId, ExecutionReference>,
}

pub enum ExecutionReference {
    Live(HolonReference),
    Deleted,
}
```

Append-only. Keyed by **ExpectedSnapshot token id**, not SourceSnapshot id. Many snapshot ids
may map to the same `ExecutionReference` (e.g. multiple pre-commit snapshots of the same holon
all resolve to the same saved holon after commit).

---

## 3. Tight Chaining Rule

> The expected snapshot of step N is the source snapshot for step N+1.

Adders implement this automatically:
1. Extract the ExpectedSnapshot from the input `TestReference` → use as source for the new ref
2. Clone that snapshot into a working holon
3. Apply the step's mutations to the working clone
4. Freeze the result as the new ExpectedSnapshot
5. Mint a new `TestReference` with (new source, new expected)

Test case authors never see this — they just pass tokens forward.

---

## 4. Commit Semantics

Commit is a **global lifecycle operation**, not a per-holon step.

**Fixture phase (add_commit_step):**
For each `FixtureHolon` in `Staged` state:
1. Clone the head snapshot
2. Create a new snapshot representing the saved state
3. Mint a new `TestReference` (SourceSnapshot = prior head, ExpectedSnapshot.state = Saved)
4. Advance `head_snapshot` to the new saved snapshot
5. Update lifecycle state to Saved
6. Append token to `FixtureHolons.tokens` — **not returned to the test case author**

**Execution phase (execute_commit):**
1. Dispatch `MapCommand::Transaction(Commit)` via `state.dispatch_command()`
2. Extract the `SavedHolons` relationship from the commit response
3. Match each returned saved holon to its commit-minted TestReference by key
4. Record `ExecutionReference::Live(saved_ref)` for each saved snapshot id

**Why this works:** If a test case author holds token T1 (staged) and commit mints T2 (saved),
the resolution process at execution time for a post-commit step using T1 is:
1. T1.SourceSnapshot.snapshot_id → look up in `snapshot_to_fixture_holon` → FixtureHolonId F1
2. F1.head_snapshot now points to T2.ExpectedSnapshot (advanced by commit)
3. T2.ExpectedSnapshot.snapshot_id → look up in `ExecutionHolons` → the saved `HolonReference`

The test author never touches T2. Their T1 token still works.

---

## 5. Source Resolution Process (Canonical)

Used by every executor for each `TestReference` it handles:

1. Extract `SnapshotId` from `SourceSnapshot.snapshot`
2. Look up owning `FixtureHolonId` in `snapshot_to_fixture_holon`
3. Retrieve `FixtureHolon.head_snapshot` (the current head, possibly advanced by commit)
4. Compute the head's `SnapshotId`
5. Look up `ExecutionReference` in `ExecutionHolons.by_snapshot_id`
6. Interpret `SourceSnapshot.state` to validate lifecycle expectations
7. Return the `HolonReference` (or `Deleted`) to operate on

The harness exposes this as a helper — executors must use it, not reimplement it.

---

## 6. Canonical Adder Sequence (Non-Negotiable Order)

For each holon affected by a step, adders must follow this exact order:

1. **Derive source** — extract ExpectedSnapshot from input token; make it the new SourceSnapshot.
   Never mutate the input snapshot.
2. **Clone** — clone the source into a working holon. This is the only thing the adder may mutate.
3. **Apply effects** — property changes, relationship edits, lifecycle transitions on the clone.
4. **Freeze expected** — the working holon becomes the ExpectedSnapshot. If there's any chance it
   could be mutated later, clone it first.
5. **Mint** — call `fixture_holons.mint_test_reference(source, expected)` — never construct
   `TestReference` directly.
6. **Register identity** — call the appropriate `FixtureHolons` method to either:
   - continue an existing `FixtureHolon` (mutation steps: with_properties, add_relationship, etc.)
   - create a new `FixtureHolon` (creation steps: stage_new_holon, stage_new_from_clone, etc.)
7. **Append step** — push the concrete `DanceTestStep` variant to `test_case.steps`.
8. **Return token** — return the new `TestReference` if the step produces a chainable holon.

### Head advancement rule

Advance the head when the step **continues the same logical holon** (mutations, abandons,
deletes). Do **not** advance the head when creating a new logical holon — create a new
`FixtureHolon` instead.

---

## 7. Canonical Executor Sequence (Non-Negotiable Order)

For each `TestReference` handled:

1. **Resolve source** — use harness helper to resolve `TestReference` to a runtime `HolonReference`.
2. **Execute** — perform the real operation using holon APIs; capture the result.
3. **Validate** — compare actual lifecycle state and content against `ExpectedSnapshot`.
4. **Record** — call `execution_holons.record(expected_snapshot_id, result)`.
   - Record against **ExpectedSnapshot token id**, never SourceSnapshot id.
   - Call exactly once per TestReference.
   - Append-only — do not overwrite.

Executors must never:
- Mint `TestReference`s
- Consult `FixtureHolons`
- Record against source snapshot ids
- Bypass the harness resolution helper

---

## 8. Harness Invariants (Non-Negotiable)

- `TestReference`s are immutable once minted
- `FixtureHolons` is the sole authority for snapshot registration, identity, and head advancement
- `ExecutionHolons` is the sole authority for runtime resolution results
- `ExpectedSnapshot` snapshots are never resolved to runtime holons
- Head redirection applies only during `SourceSnapshot` resolution
- Commit must mint new `TestReference`s (one per staged holon)
- Every executed step must record exactly once against its `ExpectedSnapshot` id
- `by_snapshot_id` is append-only

---

## 9. Step Parameters vs. Expected Outcomes

`TestReference` contains only holon-level intent (source + expected snapshot).
Everything else goes in the `DanceTestStep` struct:
- `PropertyMap` (for with_properties, remove_properties)
- `relationship_name` and target lists (for relationship steps)
- `expected_error: Option<HolonErrorKind>` — step-level failure expectation

A step with `expected_error = Some(...)` that fails as expected is a **successful test outcome**.
Execution continues. This enables negative test scenarios inline with positive ones.

---

## 10. TestCaseInit — Initialization Contract

```rust
pub struct TestCaseInit {
    pub test_case: DancesTestCase,
    pub fixture_context: Arc<TransactionContext>,  // fixture-scoped, no conductor
    pub fixture_holons: FixtureHolons,
    pub fixture_bindings: FixtureBindings,  // optional label namespace for test authors
}
```

`TestCaseInit::new(name, description)` is the **only supported entry point** into fixture-time
authoring. Always destructure the result.

`test_case.finalize(&fixture_context)` is the **only supported exit point**. It:
- Exports transient holons from the fixture context into `TestSessionState`
- Sets `is_finalized = true`
- Prevents further step additions

The execution phase imports these transient holons into the real runtime context before executing
any steps, ensuring fixture-phase state is available to executors.

---

## 11. Execution Initialization (Reference)

The execution phase setup (`init_test_runtime`):
1. Creates a `ClientHolonService`
2. Creates a `TrustChannel`-backed `DanceInitiator` from a real `SweetConductor`
3. Creates `HolonSpaceManager` with the conductor-backed initiator
4. Creates `RuntimeSession` and `Runtime`
5. Dispatches `MapCommand::Space(SpaceCommand::BeginTransaction)` to get the initial `TxId`
6. Imports transient holons from `test_case.test_session_state` into the new transaction context

The harness (`init_test_runtime` in `src/harness/helpers/test_context.rs`) owns this entirely.
Test case authors and step authors do not interact with it.
