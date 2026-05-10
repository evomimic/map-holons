# MAP Design Spec: Dances, Descriptor Affordances, and Execution Binding v1.1

**Status:** Draft  
**Author:** MAP Core / Steve Melville  
**Intent:** Specify how dances fit into the descriptor model, how dance execution binds to implementations, and how dance inputs/outputs align with MAP query/navigation data structures.  
**Scope:** Descriptor integration, dance descriptors and implementations, request/response data structures, query-algebra alignment, validation/governance, runtime dispatch, security, caching, and compatibility with existing MAP/Holochain patterns.

---

## 0) Core Synthesis

This version integrates the newer descriptor design and query design.

The main architectural synthesis is:

> Descriptors own dance affordance semantics.  
> Dance dispatch resolves through descriptor lookup.  
> Dance execution may later bind to dynamic implementations.  
> Query/navigation dances should reuse the same shared query substrate and operand structures as MAP query algebra.

This changes the framing of the older dance design in three important ways:

- `HolonDescriptor` is now the primary caller-facing surface for dance discovery
- dance inheritance/lookup must follow descriptor `Extends` flattening rules
- dance request/response payloads should align with the shared query-adjacent operand family such as `Value`, `Row`, `RowSet`, and eventually `RecordStream`

This doc therefore extends the descriptor design rather than competing with it.

---

## 1) Relationship to the Descriptor Design

The descriptor design already establishes:

- `InstanceDance` as a behavior family afforded by `HolonType` descriptors
- flattened inherited affordance lookup across `Extends`
- no override and no deletion
- `HolonDescriptor` as the primary instance-facing lookup surface
- static descriptor-local dispatch in the current phase

This dance design adds the next layer:

- richer dance descriptor metadata
- runtime binding of dance affordances to executable implementations
- governance and activation for executable implementations
- request/response structures for invocation

Interpretation rule:

- the descriptor design is authoritative for the existence and lookup semantics of dances
- this dance design is authoritative for how those descriptor-afforded dances are bound and invoked

That means this document must not reintroduce:

- a second global dance registry
- caller-side inheritance reconstruction
- freestanding dance semantics detached from descriptors

---

## 2) Foundational Assumptions

1. **Self-describing types**
   - All MAP types are holons described by descriptor holons.
   - Descriptor wrappers are thin typed views over `HolonReference`.
   - Structural and behavior affordances should be discovered from descriptors, not hardcoded registries.

2. **Dance ownership**
   - Dances are instance behaviors afforded by `HolonType` descriptors.
   - Effective dance lookup is inherited and flattened through descriptor `Extends`.

3. **Descriptor-local meaning**
   - `HolonDescriptor` owns effective dance discovery for a holon instance.
   - Query/operator semantics remain owned by `ValueDescriptor`, not by dance-specific code.

4. **Execution layering**
   - The current descriptor design stops at static dispatch surfaces.
   - Dynamic dance implementation loading is a future extension layer built on top of descriptor affordances.

5. **Query-algebra compatibility**
   - Navigation and query-oriented dances should exchange payloads using MAP query operand structures where applicable.
   - The materialized contract-shape definitions for `Value`, `Row`, and `RowSet` come from the shared operand family foundation, not from this dance spec.
   - Dance design should not introduce a parallel family of ad hoc tabular/query result structures.
   - Dances should invoke shared query substrate capabilities rather than depend on a Commands-owned query runtime.

---

## 3) Conceptual Model

At the semantic level, dances are descriptor-afforded instance behaviors:

```text
(HolonTypeDescriptor) -[AffordsInstanceDance]-> (DanceDescriptor)
```

At the execution-binding level, dance affordances may be associated with executable implementations:

```text
(HolonTypeDescriptor) -[ImplementsDance]-> (DanceImplementationDescriptor)
(DanceImplementationDescriptor) -[ForDance]-> (DanceDescriptor)
```

### 3.1 Semantic vs Execution Layers

These layers must remain distinct:

| Layer | What It Owns |
|---|---|
| Descriptor affordance layer | whether a type affords a dance |
| Implementation binding layer | what executable implementation may satisfy that dance |
| Dispatch layer | how a dance request is routed to a concrete implementation |
| Query/navigation layer | what operand/result structures are exchanged |

This prevents the mistake of treating implementation binding as if it defined the existence of a dance.

### 3.2 Behavior Resolution

When the system receives a dance invocation for a target holon:

1. resolve the target holon's `HolonDescriptor`
2. resolve the effective inherited dance affordance set
3. resolve the requested `DanceDescriptor`
4. resolve the best active implementation binding for that `(descriptor, dance)` pair
5. invoke the implementation using the defined dance ABI and operand model

This keeps dance discovery descriptor-first.

---

## 4) Dance Descriptor Model

### 4.1 Core Descriptor Kinds

This design assumes or extends the following descriptor holons:

- `DanceDescriptor`
- `DanceImplementationDescriptor`
- optional `DanceRequestDescriptor`
- optional `DanceResultDescriptor`

### 4.2 `DanceDescriptor`

`DanceDescriptor` is the semantic descriptor for a single instance behavior.

It should be treated as the dance counterpart to the other descriptor wrappers in the descriptor design.

Minimal metadata:

- `dance_name`
- `display_name`
- `description`
- optional `category`
- optional `stability`

Relationships:

- `RequestShape` -> request descriptor or request type
- `ResultShape` -> result descriptor or result type
- optional `ProducesRowSet`
- optional `ProducesValue`
- optional `ProducesSmartReferences`

### 4.3 `DanceImplementationDescriptor`

This descriptor represents a concrete executable binding for a `DanceDescriptor` on behalf of an affording type.

Suggested properties:

- `engine`
- `module_ref`
- `entrypoint`
- `abi`
- `version`
- `compat_range`
- `activation_status`
- `scope`
- `module_hash`

Relationships:

- `ForDance` -> `DanceDescriptor`
- `ForAffordingType` -> `HolonTypeDescriptor`

---

## 5) Query-Aligned Dance Data Structures

This is the most important integration with `map-queries`.

The older dance design used generic `DanceRequest` and `DanceResponse` envelopes but did not align them to MAP's emerging query/navigation operand model.

This version does.

### 5.1 Core Operand Family

Dance inputs and outputs should reuse the same conceptual operands used by MAP query/navigation layers where appropriate:

- `Value`
- `Row`
- `RowSet`
- future `Record`
- future `RecordStream`
- `SmartReference`

Interpretation rule:

- `Value`, `Row`, and `RowSet` should be read according to `shared-operand-family-foundation.md`
- this dance spec does not redefine their shape constraints
- alignment here is about contract compatibility, not about forcing one internal execution representation
- query-aligned dance execution may retain richer holon-bound or descriptor-aware state internally and materialize row-shaped results only when a contract, ABI, or operator requires them

### 5.2 Guidance by Dance Category

| Dance Category | Preferred Input/Output Shapes |
|---|---|
| Scalar/transform dance | `Value` in, `Value` out |
| Holon-local action | target holon + structured parameters, result as `Value` or structured holon result |
| Navigation dance | target holon + navigation parameters, result as `RowSet` or `SmartReference` collection |
| Query dance | query expression or algebra plan, result as `RowSet`, later `RecordStream` |
| Bulk dance | list/collection input, result as `RowSet`, list, or structured batch result |

### 5.3 Why This Matters

This avoids three different result models for:

- query execution
- navigation dances
- DAHN-driven graph exploration

Instead:

- navigation-oriented dances can return `RowSet`
- query dances can grow naturally into `RecordStream`
- distributed query surfaces can still use `SmartReference`-oriented outputs where sovereignty requires it

It also preserves room for deferred projection:

- a dance may internally retain richer holon- or relationship-bound execution state
- it may materialize `Value`, `Row`, or `RowSet` only when the ABI, result contract, or an operator boundary requires those shapes

### 5.4 Canonical Invocation and Outcome Envelope (PRO1 Foundation)

The first contract-track dance slice should stabilize the canonical dance
invocation and outcome envelope posture before descriptor-backed lookup,
dispatch-routing, operand-family alignment, and ABI finalization are fully
defined.

This PRO1 foundation owns the role boundaries between:

- dance identity
- target selection
- structured parameters
- execution context
- structured successful results
- diagnostics
- events
- failure reporting

The canonical dance execution posture in PRO1 is:

```text
DanceExecutionResult = Result<DanceOutcome, HolonError>
```

Interpretation rules:

- invocation failure is represented through `HolonError`
- successful execution returns `DanceOutcome`
- non-fatal diagnostics and emitted events are returned with a successful
  outcome
- HTTP-like response status codes are not part of the canonical PRO1 contract

### 5.5 Canonical Invocation Envelope

The canonical invocation envelope in PRO1 is:

```text
DanceInvocation {
  identity
  target
  parameters
  context
}
```

#### 5.5.1 Dance Identity

```text
DanceIdentity {
  dance_name
  dance_descriptor_ref?
}
```

Interpretation rules:

- `dance_name` is the primary invocation identity in PRO1
- `dance_descriptor_ref` is optional and non-authoritative in PRO1
- PRO1 does not define validation or lookup semantics between `dance_name` and
  `dance_descriptor_ref`
- PRO1 does not require descriptor-backed dance resolution at invocation time

#### 5.5.2 Dance Target

```text
DanceTarget =
  | None
  | One(HolonReference)
```

Interpretation rules:

- target selection is distinct from dance identity
- target selection is distinct from structured parameters
- PRO1 defines only no-target and single-target posture
- multi-target invocation posture is deferred

#### 5.5.3 Dance Parameters

```text
DanceParameters =
  | None
  | ParameterHolon(HolonReference)
```

Interpretation rules:

- structured dance parameters are conveyed through a parameter holon reference
- canonical PRO1 parameter references must be `Transient` references
- parameters are distinct from target selection
- PRO1 does not yet define final operand-family or ABI payload alignment

#### 5.5.4 Dance Context

```text
DanceContext {
  invocation_source
  capability_ref?
  affording_type_ref?
}
```

```text
DanceInvocationSource =
  | ClientCommand
  | TrustChannel
  | Internal
```

Interpretation rules:

- `DanceContext` carries invocation-time execution metadata
- `DanceContext` is distinct from dance identity, target selection, and
  structured parameters
- `invocation_source` distinguishes the ingress posture of the invocation
- `capability_ref` provides an optional slot for trust/capability/provenance
  context
- `affording_type_ref` provides an optional slot for affording-type execution
  context
- PRO1 does not define final semantics for capability enforcement,
  descriptor-backed dispatch, or binding through `affording_type_ref`

Commands-originated invocation and TrustChannel-originated invocation should
both converge on this same canonical `DanceInvocation` shape.

### 5.6 Canonical Successful Outcome Envelope

The canonical successful outcome envelope in PRO1 is:

```text
DanceOutcome {
  result
  diagnostics
  events
}
```

#### 5.6.1 Structured Success Result

```text
DanceResult =
  | None
  | Holon(Holon)
  | HolonReference(HolonReference)
```

Interpretation rules:

- PRO1 intentionally limits canonical structured success results to:
  - no result
  - a holon
  - a holon reference
- PRO1 does not canonize `HolonCollection`
- PRO1 does not retain `NodeCollection` as a canonical dance result family
- PRO1 does not yet define `Row`, `RowSet`, `Record`, or `RecordStream` as
  canonical dance result families
- collection-bearing and query-aligned result convergence is deferred to later
  work

#### 5.6.2 Diagnostics

```text
DanceDiagnostic {
  severity
  code
  message
}
```

```text
DanceDiagnosticSeverity =
  | Info
  | Warning
```

Interpretation rules:

- diagnostics are non-fatal execution notes
- diagnostics are returned only within successful outcomes
- diagnostics do not replace `HolonError`
- diagnostics should remain lightweight and machine-identifiable

#### 5.6.3 Events

```text
DanceEvent {
  event_name
  payload?
}
```

Interpretation rules:

- events are execution-side outcome artifacts returned with a successful
  outcome
- event payloads remain MAP-native
- in PRO1, event payloads may optionally point at holon-backed payloads using
  `HolonReference`
- PRO1 does not define a richer event schema family

### 5.7 Failure Reporting

Failure reporting remains `HolonError`-based.

Interpretation rules:

- `Err(HolonError)` indicates invocation failure
- `Ok(DanceOutcome)` indicates invocation success, with optional diagnostics and
  events
- new `HolonError` additions should remain minimal and precise
- existing general `HolonError` variants should be reused where they preserve
  sufficient meaning without undue ambiguity

### 5.8 Deferred Result-Family Alignment

PRO1 intentionally does not finalize the canonical multi-result or query-aligned
result family.

In particular, PRO1 defers:

- canonical collection-bearing dance result structures
- query-aligned `RowSet` result posture
- later `Record` / `RecordStream` result posture
- final composable or pipeline-oriented multi-result forms

The important semantic constraint remains:

> Dance results should converge with MAP query/navigation operand and result
> structures rather than hardening a second incompatible family.

---

## 6) Validation and Query Semantics Inside Dances

The new descriptor and query designs imply a strong boundary:

- dances do not own value/operator semantics
- `ValueDescriptor` owns value validation and operator application

So if a dance needs to:

- validate input values
- apply filters
- compare values
- interpret query predicates

it should rely on descriptor-backed value semantics rather than custom per-dance logic wherever possible.

Examples:

- a search dance should use `ValueDescriptor.supports_operator()` and `apply_operator(...)`
- an edit dance should use descriptor-driven validation for candidate values
- a navigation/filter dance should compile or execute against descriptor-aware algebra rather than handwritten property predicate code

This keeps dance logic from becoming a semantic dumping ground.

---

## 7) Import and Schema Additions

### 7.1 Descriptor Relationships

The canonical relationships should align with descriptor terminology:

- `(HolonTypeDescriptor) -[AffordsInstanceDance]-> (DanceDescriptor)`
- `(HolonTypeDescriptor) -[ImplementsDance]-> (DanceImplementationDescriptor)`
- `(DanceImplementationDescriptor) -[ForDance]-> (DanceDescriptor)`

Optional:

- `(DanceDescriptor) -[RequestShape]-> (TypeDescriptor or ValueDescriptor-backed request shape)`
- `(DanceDescriptor) -[ResultShape]-> (TypeDescriptor or result descriptor)`

### 7.2 Descriptor Inheritance Rules

Dance affordances must follow the same inheritance rules as other descriptor affordances:

- flatten across `Extends`
- no override
- no deletion
- duplicate redeclaration is invalid

This is inherited from the descriptor design and should not be restated differently elsewhere.

---

## 8) Runtime Binding and Dispatch

### 8.1 Current-Phase Compatibility

The descriptor design currently promises static, descriptor-local dispatch.

Therefore this dance design should be interpreted in two phases:

#### Phase A: Static Descriptor-Local Dispatch

- dance affordances are discovered from `HolonDescriptor`
- dispatch goes to handwritten Rust implementations
- no dynamic module loading is required

#### Phase B: Dynamic Implementation Binding

- descriptor-afforded dances are resolved to `DanceImplementationDescriptor`s
- implementations may be loaded dynamically through WASM/WASI or other engines
- governance and activation determine which implementations are active

This preserves compatibility with the current descriptor roadmap while keeping the larger dance vision intact.

### 8.2 Dispatch Algorithm

Given `(target, dance_name, ctx)`:

1. resolve `target.holon_descriptor()`
2. resolve `get_instance_dance_by_name(dance_name)`
3. resolve candidate active `DanceImplementationDescriptor`s for the effective affording type
4. choose one deterministically by:
   - scope precedence
   - exact version/compatibility
   - policy eligibility
   - stable tiebreaker
5. load or reuse the executable implementation
6. invoke with the dance ABI and operand model
7. validate and return a `DanceExecutionResult`

If Phase A only is implemented, step 3 collapses to a static descriptor-local dispatch table.

---

## 9) ABI

### 9.1 Goals

- stable host/implementation contract
- clear operand/result model
- deterministic execution
- compatibility across engines

### 9.2 Core Shape

The dance ABI should explicitly accommodate the canonical invocation and
outcome posture defined in PRO1, while preserving room for later query-aligned
operand expansion.

Inputs:

- `dance_name`
- optional `dance_descriptor_ref`
- `target`
- `parameters`
- `context`

Outputs:

- `Result<DanceOutcome, HolonError>`
- `result`
- `events`
- `diagnostics`

Where:

- PRO1 canonical success results are limited to `None`, `Holon`, and
  `HolonReference`
- later operand-family and collection/result-family expansion remains a
  subsequent layer

### 9.3 ABI Constraint

The ABI should not require every dance to serialize into one opaque JSON blob when stronger MAP-native operand structures are available.

Opaque transport encoding is fine, but the semantic model should still distinguish:

- invocation failure versus successful outcome
- dance identity, target selection, parameters, and execution context
- single-result payloads versus later query-aligned collection/result families
- structured diagnostic outcomes

---

## 10) Validation Rules

### 10.1 Import-Time / Schema-Time

- `impl-consistency`
  - if `(T ImplementsDance impl)` then `(impl ForDance)` must refer to a dance effectively afforded by `T`
- `single-active-impl`
  - at most one active implementation for a deterministic `(affording type, dance, scope, version resolution path)` slot
- `engine-fields-required`
  - required fields vary by engine
- `descriptor-inheritance-consistency`
  - duplicate inherited dance redeclarations are invalid
- `request-result-shape-consistency`
  - if a dance declares `ResultShape`, its ABI/result kind must be compatible with that shape

### 10.2 Activation-Time

- `abi-compat`
- `module-integrity`
- `policy-eligibility`
- optional request/result-shape conformance checks

### 10.3 Runtime Semantics Checks

- query/navigation dances returning `RowSet` or `SmartReference` collections should preserve the semantics promised by their declared shapes
- filter/query-oriented dances should fail on unsupported descriptor operators rather than silently reinterpret predicates

---

## 11) Security, Provenance, and Audit

No major conceptual changes here, but descriptor integration clarifies what is being audited.

Every dispatch should log at least:

- target holon
- resolved affording descriptor
- resolved `DanceDescriptor`
- resolved implementation
- module hash / builtin identity
- outcome / failure classification
- duration / resource usage

This makes it possible to distinguish:

- semantic dance identity
- concrete executable binding

which is essential once multiple implementations can satisfy one dance affordance.

---

## 12) Performance and Caching

Separate caches should exist for:

- holon state
- descriptor lookup/effective affordance lookup
- query/navigation runtime structures such as `ResolvedType` or planner artifacts where still used
- executable modules/instances

Important integration rule:

- dance execution caches must not obscure descriptor changes that alter effective affordances or request/result semantics

---

## 13) Compatibility and Migration

### 13.1 With the Current Descriptor Plan

Near-term rollout should be:

1. land descriptor-local dance affordance lookup on `HolonDescriptor`
2. keep execution static and Rust-local first
3. align dance request/result structures with query/navigation operands
4. introduce `DanceImplementationDescriptor` and dynamic binding later

### 13.2 With Query Architecture

Navigation and query dances should evolve toward:

- algebra-backed execution
- descriptor-aware predicate semantics
- `RowSet` / `RecordStream` compatible outputs
- shared query substrate reuse across TS invocation, trust-channel flows, and dance-initiated execution

This lets query support emerge from the same substrate rather than from a Commands-owned or query-only runtime.

---

## 14) Open Questions

- should `DanceDescriptor` request/result shapes point to `HolonType` descriptors, `ValueDescriptor` structures, or a dedicated request/result descriptor family?
- when should `RowSet` give way to `RecordStream` in public dance results?
- should distributed query dances declare `SmartReference`-only result contracts explicitly?
- how much of dance invocation should be modeled as algebra-emitting behavior versus opaque module execution?
- what minimum host-import surface is needed for query/navigation dances versus side-effecting dances?

---

## 15) Acceptance Criteria

- dances are discovered from descriptor affordances, not a global registry
- effective dance lookup is inherited and flattened through descriptor semantics
- dance invocation structures align with MAP query/navigation operand models
- query/filter semantics used by dances rely on descriptor-backed operator/value semantics
- dances can consume the shared query substrate without depending on Commands as the semantic owner
- the design supports both current static descriptor-local dispatch and later dynamic implementation binding
- implementation binding, governance, and audit semantics remain explicit and deterministic

---

## 16) Next Steps

1. align core schema names and relationships with descriptor terminology:
   - `DanceDescriptor`
   - `AffordsInstanceDance`
   - `DanceImplementationDescriptor`
2. implement descriptor-local dance lookup on `HolonDescriptor`
3. define the canonical dance invocation/result operand model
4. align navigation/query dances with `Value` / `Row` / `RowSet` and future `RecordStream`
5. defer dynamic module loading until after static descriptor-local dispatch is stable
6. add governance/activation and module-binding only after the descriptor-owned affordance layer is working end to end
