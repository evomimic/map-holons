# AGENTS.md

Baseline guidance for AI coding agents working in the MAP Holons repository.

This file provides shared project orientation and engineering constraints that apply across coding
agent harnesses and task types. Harness-specific instructions (`CLAUDE.md`,
`AGENTS.override.md`, local notes) may add approval, sandbox, editing, and interaction rules. Follow
the more specific instruction unless it violates repository architecture or safety constraints.

Act as a careful senior engineer in an active pre-production codebase: understand before changing,
keep edits scoped, and prefer current architectural alignment over obsolete patterns.

MAP Holons is the foundational layer of the Memetic Activation Platform: a Holochain-based system
for storing, retrieving, querying, and evolving self-describing active holons. Holons model runtime
instances, types, relationships, values, schemas, commands, dances, and other platform concepts.

## Core Invariants

These are the rules most likely to be violated by a locally plausible change:

* The repository mirrors runtime execution contexts. If code cannot run together, do not mix it in
  the filesystem, workspace, or dependency graph.
* `shared_crates/` is WASM-first. Do not add threading, OS/filesystem assumptions, native-only
  runtime assumptions, or host-only dependencies.
* `TransactionContext` is the transaction-scoped execution surface. Route runtime mutation, lookup,
  and commit through it; do not introduce parallel operation APIs.
* Runtime references are bound, self-resolving handles. Do not serialize bound runtime reference
  handles directly or thread context through ordinary reference operations.
* Wire types stop at transport boundaries. Bind wire → runtime at ingress and project runtime →
  wire at egress; do not leak `*Wire` types into domain execution.
* Do not bypass the Reference Layer to access storage/DHT, or add parallel ingress paths where one
  is designated.
* Do not build/check/test the root Cargo workspace directly. Prefer root npm scripts; use
  workspace-specific or `--manifest-path` Cargo commands only when intentional.
* Do not hand-edit canonical MAP Core Schema import JSON unless explicitly instructed.
* Diagnose before changing. Keep edits scoped and explicit.

## Sources of Truth

Consult before making architectural assumptions:

* `map-dev-docs` is expected to be available as a sibling checkout or equivalent local workspace when present. It is the authoritative source of truth for MAP design specs, architectural intent, and naming. In `map-holons`, all behavioral, architectural, and cross-boundary guidance must come from `map-dev-docs`; do not create parallel doctrine in local files such as `CONTEXT.md`. Local notes may only summarize, index, or point to the authoritative specs, and they must remain strictly subordinate to them. If any local file conflicts with `map-dev-docs`, treat the docs repository as decisive and call out the conflict instead of choosing a local interpretation. 
* `ARCHITECTURE.md` — workspace and execution-context boundaries.
* `CONTEXT.md` and folder-local context/design notes — current vocabulary and subsystem intent.
* `README.md` — setup and developer documentation.
* Active issue/spec/PR text — task-specific intended behavior.

Call out conflicts instead of silently choosing an interpretation. Search existing code and docs
before creating a new module, path, or architectural seam.

`host/import_files/map-schema/core-schema/` contains canonical generated/curated MAP Core Schema
JSON imports. Treat these as schema source-of-truth inputs. Do not hand-edit them unless explicitly
instructed. If a task appears to require changes there, ask the user to update or regenerate the
schema source files.

## Execution Contexts

| Context       | Directory        | Constraint                                         |
| ------------- | ---------------- | -------------------------------------------------- |
| hApp / WASM   | `happ/`          | Holochain guest code and zomes                     |
| Host / native | `host/`          | Native Rust, Tauri, TypeScript, orchestration, SDK |
| Shared        | `shared_crates/` | WASM-safe libraries consumed by hApp and host      |
| Tests         | `tests/`         | Sweettest, Tryorama, fixtures, harnesses           |

Placement heuristic:

> Could this run inside Holochain WASM?

If not, it belongs in `host/` only. Tests do not relax execution-context constraints.

Shared crates are path dependencies consumed separately by the host and hApp workspaces, not a
standalone workspace with one universal feature set. A host/native build does not prove WASM
safety. Validate shared-crate changes through the hApp workspace when the crate is
guest/WASM-reachable:

```sh
npm run check -w map-happ
```

The root `npm run check` includes this check. It does not prove WASM safety for a shared crate that
is not reachable from the hApp workspace; review such crates explicitly.

## Runtime Architecture

Preserve the layering:

```text
Client / Host
  -> Choreography / Dances / Commands
    -> Reference Layer
      -> Shared Objects
        -> Storage / DHT
```

Do not bypass the Reference Layer to access storage directly.

For command/runtime work, preserve the boundary pipeline:

```text
wire at IPC/transport ingress
  -> bind to runtime/domain types
    -> execute with bound runtime handles
      -> project to wire at egress
```

Runtime code should not operate directly on raw wire structs after binding.

The host command pipeline intentionally separates:

* `host/crates/map_commands_wire` — serializable IPC command/result types.
* `host/crates/map_commands_contract` — domain command/result contract.
* `host/crates/map_commands_runtime` — binding, dispatch, and runtime session logic.

## Transaction and Reference Model

At the Commands layer, client JSON IPC ingress is partitioned into `SpaceCommand`, `TransactionCommand`, and `HolonCommand`. That partitioning is an ingress concern only; it should not leak upward into the shared-object API surface or downward into unrelated runtime code.

In the shared objects layer, prefer `HolonReference` and the established `ReadableHolon` / `WritableHolon` traits over operating on reference variants directly, unless a specific lifecycle constraint requires `TransientReference`, `StagedReference`, or `SmartReference`. Ordinary holon operations should be invoked through the reference itself, not by passing a separate `TransactionContext` into each call.

`HolonReference` and its variants are bound, self-resolving handles. Their ordinary read/write operations resolve through the transaction context already carried by the handle. Do not reintroduce APIs that require supplying `TransactionContext` for normal `HolonReference` or `HolonCollection` operations.

Do not add parallel mutation, lookup, or commit surfaces that bypass the established reference layer or transaction lifecycle policy.

Reference phases:

| Type                 | Backing                       | Read | Write | Commit |
| -------------------- | ----------------------------- | ---: | ----: | -----: |
| `TransientReference` | transient manager / memory    |  yes |   yes |     no |
| `StagedReference`    | nursery                       |  yes |   yes |    yes |
| `SmartReference`     | saved cache / persisted holon |  yes |    no |     no |

Use the established high-level reference types and traits:

* `HolonReference`
* `ReadableHolon`
* `WritableHolon`

Do not ad-hoc cast phases or invent alternate lifecycles.

## Build, Test, and Dependencies

The root workspace exists for IDE support and dependency coordination, not builds. Prefer root npm
entrypoints:

```sh
npm run check          # host + hApp WASM checks + web typecheck
npm run fmt:check
npm run test:unit
npm run build
npm run sweetest       # Holochain integration/dance/loader/e2e; requires Nix
```

Use the narrowest relevant validation while iterating and broader checks before completion.

| Change area                                             | Suggested validation                                             |
| ------------------------------------------------------- | ---------------------------------------------------------------- |
| Rust formatting                                         | `npm run fmt:check`                                              |
| Shared/host/hApp compile behavior                       | `npm run check`                                                  |
| Unit-level Rust/TS behavior                             | `npm run test:unit`                                              |
| Holochain guest, dance, loader, or integration behavior | `npm run sweetest`                                               |
| TypeScript SDK                                          | package scripts under `host/map-sdk` or root `npm run test:unit` |
| Web/UI behavior                                         | root web test/typecheck scripts                                  |

Report checks that could not be run. Treat long-running, network-dependent,
dependency-installing, or system-modifying commands according to the active harness approval policy.

Sweettest environment:

```sh
nix develop
npm install
npm run sweetest
exit
```

Each build workspace has its own authoritative lockfile:

* `happ/Cargo.lock` — hApp/WASM resolution.
* `host/Cargo.lock` — native resolution.

Differences are expected. Commit both workspace lockfiles; never delete them during cleanup. Do not
run `cargo update` casually or pin a transitive dependency in `Cargo.toml` solely to work around
MSRV. For an intentional transitive adjustment, run from the affected workspace and review the
lockfile diff:

```sh
cargo update -p crate_name@bad_version --precise wanted.version
```

Holochain dictates MSRV. Keep dependency changes deliberate, auditable, and reproducible.

## Editing Discipline

* Keep changes scoped. Avoid opportunistic cleanup, unrelated refactors, and new patterns where an
  established one exists.
* Do not restructure workspaces or move modules across execution-context boundaries unless the task
  requires it.
* Backwards compatibility with obsolete paths is not a general requirement in this pre-production
  codebase. Do not remove compatibility code as incidental cleanup; when it conflicts with current
  architecture, surface the conflict and remove or replace it deliberately.
* Do not directly edit files under `host/import_files/map-schema/core-schema/` as part of normal
  implementation work. Ask the user to update or regenerate schema source files unless direct edits
  were explicitly requested.
* Call out and propagate cross-boundary changes:

    * shared API → host/hApp consumers
    * wire contract → binders/SDK fixtures
    * command contract → runtime/SDK
    * descriptor/type-system → loader/schema/tests
* When debugging, separate confirmed causes from hypotheses. Do not produce a broad speculative
  patch or claim an exact root cause without evidence.

## Rust Conventions

* Prefer `use holons_prelude::prelude::*;`.
* Use `Result<T, HolonError>` in shared/host logic and `ExternResult<T>` in zome externs.
* Keep MAP newtypes (`MapString`, `MapBoolean`, `MapInteger`, `BaseValue`, etc.) at
  persistence/API boundaries; use primitives internally where appropriate.
* Keep `lib.rs` and `mod.rs` to wiring and re-exports, not business logic or type definitions.
* Prefer descriptive names. Avoid abbreviations unless they are already idiomatic in this codebase.
* Comments should explain intent, invariants, or why a step exists, not narrate code.
* Use `///` doc comments for public items.
* Preserve existing descriptive comments unless explicitly asked to remove them.

Comment style:

```rust
// Snapshot relationship members before resolving targets to avoid re-entrant locks.
```

Avoid comments that merely restate the next line of code.
