# MAP Repository Architecture

## 1. Core Mental Model (The Headline Idea)

**The repository is a mirror of runtime execution contexts.**

Everything else follows from this.

MAP is designed so that the *filesystem structure directly reflects where code can execute at runtime*. If two pieces of code can compile together, they can run together. If they cannot run together, they are isolated at the repository and workspace level.

This alignment is intentional and foundational: it prevents entire classes of runtime errors, dependency leaks, and architectural drift.

---

## 2. Primary Execution Contexts

Ignoring tests for the moment, MAP has **two primary execution contexts**:

1. **hApp** — Holochain App (WASM execution context)
2. **Host** — Native execution context (Rust + TypeScript)

Each execution context has:
- its **own workspace**
- its **own dependency resolution**
- its **own runtime constraints**

There is no “mixed” execution context. Boundaries are strict by design.

---

## 3. Workspace-to-Execution-Context Mapping

| Execution Context | Workspace | Purpose |
|------------------|-----------|---------|
| hApp (WASM) | `happ/` | Everything compiled into a Holochain App |
| Host (Native) | `host/` | Everything running natively (CLI, Conductura, HX/UI, orchestration) |
| Coordination | root workspace | IDE support + dependency version coordination only |
| Test | `test/` | Sweettest, Tryorama, fixtures, executors |

**Important:**  
The root workspace is *never* a build target. Builds must always be run from `happ/` or `host/`.

---

## 4. The Three Crate Classes

MAP code falls into exactly **three crate classes**, distinguished by where they live.

### 4.1 hApp-only Crates
- Live under `happ/`
- Compiled to WASM
- Subject to Holochain’s strict runtime constraints (which are more restrictive than generic Rust→WASM)
- Include:
    - Zomes
    - Workers
    - hApp/DNA packaging logic and generated artifacts (e.g. `dna.yaml`, `happ.yaml`)

**Mental rule:**  
If it touches Holochain APIs, it belongs here.

---

### 4.2 Host-only Crates
- Live under `host/`
- Full native Rust environment
- May use threads, async runtimes, filesystem access, OS APIs
- Include:
    - Conductura (host command and integration layer)
    - CLI and command dispatch
    - Receptors and external integrations
    - Host orchestration and runtime setup
    - TypeScript-based Human Experience (HX/UI) layer

**Mental rule:**  
If it requires native capabilities, it belongs here.

---

### 4.3 Shared Crates
- Live under `shared_crates/`
- **Not their own workspace**
- Compiled into both hApp and Host
- Must obey the **strictest common denominator** of runtime constraints (hApp-safe)

Practically this means:
- No multithreading
- No OS assumptions
- No host-only dependencies
- Conservative feature usage

**Mental rule:**  
Shared crates are WASM-first, even when used on the host.

---

## 5. Why Shared Crates Are Not a Workspace

Shared crates are intentionally *not* placed in their own workspace.

Reasons:
- They are compiled separately in both `happ` and `host`
- Feature resolution occurs in the consuming workspace
- Different feature sets may be enabled in hApp vs Host
- This prevents accidental leakage of host-only features into WASM

Shared crates should be thought of as *pure libraries whose behavior is shaped by where they are linked*.

---

## 6. Dependency Resolution and Cargo.lock Files

Each build workspace maintains its own dependency lockfile:

- `happ/Cargo.lock` — authoritative for hApp/WASM builds
- `host/Cargo.lock` — authoritative for native builds

The root workspace does **not** produce a build lockfile.

### Dependency Version Coordination
- The root workspace centralizes *declared dependency versions* via `workspace = true`
- This supports IDE tooling and expresses version intent
- Actual resolution and locking happens per workspace

Version differences between `happ/Cargo.lock` and `host/Cargo.lock` are **expected and valid**, reflecting different runtime needs.

---

## 7. Conductura’s Role in the Architecture

Conductura is a **host-side command and integration subsystem**.

It currently provides:
- Command dispatch and mapping
- Configuration setup
- Receptor registration and management
- Integration bridges (including the Holochain receptor)
- Host runtime orchestration

Conductura never runs inside WASM and should always be treated as host infrastructure, even when it interacts closely with hApps.

---

## 8. Tests as a First-Class Architectural Layer

The `test/` directory mirrors real execution contexts rather than bypassing them.

It includes:
- Sweettest and Tryorama
- Test harnesses and execution support
- Fixtures and executors that model real runtime behavior

**Key principle:**  
Tests do not relax architectural constraints — they enforce them.

---

## 9. Build Discipline (Non-Negotiable Rules)

To preserve isolation and reproducibility:

1. All builds must use the provided build scripts
2. Builds must be executed from workspace roots (`happ/` or `host/`)
3. Never run `cargo build` or `cargo test` from the repository root
4. `Cargo.lock` files are workspace-local and intentional

Violating these rules will lead to non-reproducible builds and subtle dependency errors.

---

## 10. One-Sentence Summary for Developers

If you’re unsure where code belongs, ask:

**“Could this run inside Holochain WASM?”**

If the answer is **no**, it does not belong in `shared_crates/` or `happ/`.

That single heuristic will prevent most architectural violations.