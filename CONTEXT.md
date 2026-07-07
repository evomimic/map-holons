# MAP Holons

This context defines the shared language for MAP holon runtime, query, navigation, dance, and command work in this repository.

## Language

**HolonReference**:
The canonical singular bound runtime handle for a holon within a transaction.
_Avoid_: Node id, raw holon pointer

**BoundHolonCollection**:
The canonical plural bound runtime shared type, represented as a typed Rust facade over a HolonReference pointing at a holon-backed collection.
_Avoid_: RowSet, HolonCollection as a cross-surface contract, BoundHolonCollectionReference

**Members Collection**:
The relationship-backed HolonCollection handle exposed by a BoundHolonCollection facade for its member holon references.
_Avoid_: Direct Vec as primary BoundHolonCollection storage

**ItemType**:
The optional relationship from a BoundHolonCollection to the descriptor for its intended member holon type.
_Avoid_: Required PRO3 member conformance check

**VariableName**:
A plan and binding-layer symbol used to name values in navigation query execution.
_Avoid_: BoundHolonCollection property

**ExecutionPlan**:
A replayable MAP navigation/query view represented as holon-backed algebra operation nodes and accessed through typed Rust facades.
_Avoid_: Plain query DTO, row pipeline

**Output Binding**:
The variable name on an ExecutionPlan that selects the externally returned non-project bound result.
_Avoid_: Implicit final binding

**New-World Query Contract**:
The descriptor-aware, bound-first future query/navigation direction built around HolonReference, HolonCollection, descriptor-afforded Dances, and later holon-backed ExecutionPlans.
_Avoid_: Legacy query bridge, command-owned query envelope

**Runtime Shared Type**:
A canonical value or reference family reused across MAP surfaces without owning a surface's request or response envelope.
_Avoid_: Surface envelope, command wrapper

**Runtime Envelope**:
A surface-owned request or response container for commands, dances, future navigation surfaces, or trust-channel transport.
_Avoid_: Runtime shared type

**Descriptor-Backed Navigation Dance**:
A future descriptor-afforded Dance that performs navigation over holon-native runtime shapes.
_Avoid_: Transaction query command, NodeCollection as future algebra substrate

**Transient Execution Artifact**:
A future transaction-scoped transient holon created during read-only navigation work to preserve bound-first intermediate or result state.
_Avoid_: Staged mutation, undoable command result

**NavigationBindingSet**:
The in-memory internal bound-first closure object for new-world navigation/query execution, carrying named holon bindings, bound collections, and topology/provenance needed for composition.
_Avoid_: RowSet, a loose symbol table only

**NavigationTopology**:
The internal provenance/topology portion of a NavigationBindingSet that preserves how bound values were produced and related.
_Avoid_: Public query result contract

**ExecutionPlanReference**:
A typed Rust facade over a HolonReference pointing at a holon-backed ExecutionPlan, without descriptor validation in PRO3.
_Avoid_: Raw HolonReference when the plan role matters

**Old-World Relationship Traversal Types**:
Deprecated compatibility types retained only for the existing `query_relationships` and `fetch_all_related_holons` dance path.
_Avoid_: New query design foundation, Legacy-prefixed renames

**Retired Query Envelopes**:
The removed transaction-level query request/result contract family, including QueryRequest, QuerySpec, QueryResult, QueryResultData, and QueryDiagnostic.
_Avoid_: Compatibility resurrection, replacement query command seam

**Spec Revision Session**:
A coherent design-update session that batches fine-grained decisions before applying one version bump per affected source spec.
_Avoid_: Per-decision spec version bumps

**PlanNode**:
A holon-backed structural node in an ExecutionPlan that organizes one or more plan steps.
_Avoid_: Result node, graph node

**PlanStep**:
A holon-backed navigation/query operation such as seed, expand, filter, project, distinct, order, skip, or limit.
_Avoid_: Command action, dance step

**Step Subtype Facade**:
A typed Rust facade over a HolonReference for a specific PlanStep variant such as ExpandStep or ProjectStep.
_Avoid_: Inline step DTO

**Pipeline Steps**:
The ordered relationship-backed HolonCollection from a pipeline PlanNode to its PlanStep holons.
_Avoid_: SequenceIndex property, linked-list step ordering

**RootNode**:
The relationship from an ExecutionPlan to the PlanNode that starts execution.
_Avoid_: Inline root node DTO

**Algebra Operation**:
A navigation/query operation represented by a PlanStep.
_Avoid_: Commands-layer action, Query Command

**Step Parameter**:
A value needed by a PlanStep, modeled as a property when scalar and as a relationship when holon-reference-valued.
_Avoid_: Inline DTO field

**Bound-First Operation**:
A navigation/query operation that consumes and produces HolonReference or BoundHolonCollection values rather than row-shaped projections.
_Avoid_: Row-native operator

**Deferred Query Validation**:
The PRO3 posture that query, plan, and expand contract validation touchpoints may be named but are not enforced in this issue.
_Avoid_: Descriptor-backed enforcement in PRO3

**FilterExpression**:
A holon-backed predicate component combined by a FilterStep to preserve or remove members from a bound collection.
_Avoid_: Freestanding query-owned operator semantics

**BooleanConnective**:
The single connective used by a FilterStep to combine its FilterExpressions.
_Avoid_: Nested predicate tree for PRO3

**Query Result**:
The future outcome of descriptor-backed navigation work, which should be holon-native and bound-first unless a later projection boundary explicitly defines a materialized shape.
_Avoid_: Query expression

**Materialized Projection**:
A future projection shape produced when a descriptor-backed projection, ABI, or serialization boundary requires values.
_Avoid_: Internal execution state

**ProjectStep**:
The future materialization boundary that converts bound navigation state into descriptor-defined projection output.
_Avoid_: Implicit row materialization by order, distinct, skip, or limit

**Source Adapter**:
A format-specific translator that lowers authored or imported content into the Canonical Holon IR or projects it out to a concrete output format.
_Avoid_: Semantic core, IR owner

**Validation Layer**:
A named responsibility boundary for diagnostics, such as syntax, IR structural, schema-aware, or runtime-loader boundary validation.
_Avoid_: Unclassified diagnostic bucket

**Diagnostic Origin**:
The source location, symbol, or authored/imported element that a diagnostic should be attributed to.
_Avoid_: Validation responsibility category

**Canonical Holon IR**:
The source-neutral semantic middle shared by MAP schema tooling, validation, diffing, code generation, and future editor services.
_Avoid_: JSON model, TDL AST, loader DTO

**Semantic Diff**:
A review-oriented comparison derived from valid Canonical Holon IR models rather than concrete source text.
_Avoid_: JSON text diff, parser recovery diff

**Semantic Fidelity Check**:
A source-neutral comparison of normalized Canonical Holon IR models used to verify compile/decompile or adapter round-trips without treating formatting, ordering, or shorthand syntax as semantic differences.
_Avoid_: Textual round-trip equality

**Runtime Loader Projectability**:
The Issue 578 boundary check that Canonical Holon IR can be projected into the existing loader/import shape without changing loader behavior or creating a new runtime import path.
_Avoid_: Loader unification, runtime validation rewrite

**TypeKind-Compatible Inheritance**:
The schema-authoring rule that a type descriptor may extend another type descriptor only when both descriptors have the same TypeKind.
_Avoid_: Non-extensible descriptor rule as the primary inheritance test

**Projected TypeKind**:
The TypeKind derived from a descriptor's source-neutral semantic kind for validation and comparison.
_Avoid_: Treating authored `instance_type_kind` as the sole semantic authority

**Relationship Pair Completeness**:
The schema-authoring invariant that every declared relationship descriptor has exactly one inverse relationship descriptor paired with it.
_Avoid_: Optional inverse metadata for declared relationships

**Optional Property Suffix**:
The `?` suffix on a descriptor property name that marks the property as optional in schema authoring.
_Avoid_: Treating missing required properties as implicitly optional

**Required Slot Table**:
The fixed Issue 578 validation table that names required semantic slots per descriptor kind without deriving optionality from the full meta-descriptor graph.
_Avoid_: Full meta-schema requiredness engine

**Post-Lowering Requiredness**:
The rule that required descriptor slots are validated on Canonical Holon IR after a source adapter has applied only the explicit defaults and conveniences of its source format.
_Avoid_: Inventing semantic defaults during IR validation

**Closed-World Authoring Uniqueness**:
The Issue 578 uniqueness boundary that checks duplicates only inside the model being validated, not across the persisted MAP runtime or broader social graph.
_Avoid_: Global MAP uniqueness validation

**Blocking Semantic Diagnostic**:
An Issue 578 validation finding that makes the Canonical Holon IR unsuitable for successful check, compile, or semantic diff.
_Avoid_: Treating invalid schema semantics as warnings

**Effective Descriptor View**:
The flattened descriptor structure produced by following a descriptor's Extends chain and combining inherited structural declarations and affordances.
_Avoid_: Local-only descriptor surface

**Descriptor Flattener**:
An existing descriptor-layer operation that computes effective inherited descriptor members, such as instance properties, instance relationships, commands, or dances.
_Avoid_: Reimplemented inheritance flattening in schema tooling

**Effective Key Rule**:
The key rule selected for a descriptor or instance after resolving authored `UsesKeyRule`, applicable `Extends` lineage, and `DescribedBy`/type fallback according to MAP schema key-generation semantics.
_Avoid_: General inherited descriptor flattening

**Descriptor Semantics Service**:
A small source-neutral service for descriptor-shaped graph rules that must be shared by TDL IR validation and runtime descriptor behavior.
_Avoid_: Broad descriptor engine, ad hoc schema-tooling clone

## Relationships

- Query PRO3 removes the transaction-level query envelope family rather than replacing it with a new command-owned query envelope.
- The only retained old-world query compatibility surface is the deprecated relationship traversal dance path: `query_relationships`, `fetch_all_related_holons`, and their `Node` / `NodeCollection` / `QueryPathMap` / `QueryExpression` support types.
- Future descriptor-backed navigation behavior belongs in descriptor-afforded Dances and later Query PRS / Dance PRS work.
- A **Runtime Envelope** may carry **Runtime Shared Types** but is not itself a **Runtime Shared Type**.
- An **ExecutionPlan** has an **Output Binding** for non-project results.
- An **ExecutionPlan** reaches its starting **PlanNode** through **RootNode**.
- A pipeline **PlanNode** contains ordered **Pipeline Steps**.
- **PlanNodes**, **PlanSteps**, and **Step Subtype Facades** are holon-backed in PRO3.
- An **Algebra Operation** is represented as a **PlanStep**, not as a Commands-layer action.
- A **PlanStep** carries **Step Parameters** as properties or relationships on the step holon.
- Most **PlanSteps** are **Bound-First Operations**.
- Most **PlanSteps** consume and produce a **NavigationBindingSet**.
- A **NavigationBindingSet** is query-internal in PRO3 and is not a Commands, Dances, or SDK result contract.
- A **NavigationBindingSet** contains symbol lookup plus **NavigationTopology**, not only a variable map.
- A **BoundHolonCollection** is itself the holon-backed typed facade, and its member references live in a relationship-backed **Members Collection**.
- A **BoundHolonCollection** may carry an optional **ItemType** relationship.
- A **VariableName** belongs to **PlanSteps**, **Output Binding**, and **NavigationBindingSet**, not to **BoundHolonCollection**.
- An **Expand** step extends a **NavigationBindingSet** with target **BoundHolonCollection** bindings and any topology/provenance needed for later composition.
- **Deferred Query Validation** applies to descriptor conformance, plan structure checks, and Expand relationship contract enforcement in PRO3.
- A **Filter** step consumes a **NavigationBindingSet** and produces a filtered **NavigationBindingSet**.
- A **Filter** step contains one or more **FilterExpressions** combined by exactly one **BooleanConnective** in PRO3.
- **Distinct**, **OrderBy**, **Skip**, and **Limit** should preserve **NavigationBindingSet** as their carrier in PRO3.
- A future **ProjectStep** converts a **NavigationBindingSet** into a **Materialized Projection** only after descriptor-backed navigation work defines that projection contract.
- Without a future **ProjectStep**, navigation results should remain holon-native and selected by the **Output Binding**.
- **Old-World Relationship Traversal Types** may remain for compatibility, but **New-World Query Contract** design must not depend on them.
- A **Spec Revision Session** closes when the team produces a stable artifact for one coherent design slice, such as a revised issue body.
- A **Source Adapter** owns format-specific parsing and syntax validation; the **Canonical Holon IR** owns source-neutral semantics.
- A **Validation Layer** may classify diagnostics from adapters or derived semantic services, but adapter-specific validation responsibilities must not contaminate **Canonical Holon IR** semantics.
- A diagnostic's **Validation Layer** identifies which responsibility boundary failed; its **Diagnostic Origin** identifies where the author or tool should look.
- A **Semantic Diff** compares only valid **Canonical Holon IR** models; invalid inputs produce layered diagnostics instead of partial semantic differences.
- A **Semantic Fidelity Check** compares canonical semantics, not source formatting, JSON field order, or equivalent adapter shorthand.
- **Runtime Loader Projectability** checks whether schema-authoring semantics can flow into the existing loader/import shape without changing loader behavior.
- **TypeKind-Compatible Inheritance** allows multi-step `Extends` chains, but rejects inheritance across TypeKind boundaries.
- **Projected TypeKind** is used for **TypeKind-Compatible Inheritance**; authored `instance_type_kind` values are preserved and validated against the projection when present.
- **Relationship Pair Completeness** requires every declared relationship to have exactly one inverse and every inverse relationship to point back to a declared relationship.
- **Optional Property Suffix** is the authoring signal for property optionality; properties without the suffix are required by the corresponding meta-descriptor.
- An **Effective Descriptor View** includes inherited instance properties, instance relationships, commands, dances, key rules, operators, and related structural declarations according to each descriptor family.
- A **Descriptor Flattener** is the authoritative implementation for effective inherited descriptor members when inherited member validation is in scope; TDL R4 defers general member flattening.
- **Effective Key Rule** resolution is the narrow TDL R4 inheritance exception because Airtable already performs key generation by walking `Extends` and, when needed, `DescribedBy`/type fallback.
- A **Descriptor Semantics Service** is deferred for TDL R4 unless a required validation cannot be expressed without it.
- The **Required Slot Table** is the R4 source for descriptor-kind requiredness checks.
- **Post-Lowering Requiredness** keeps source-format defaults in the adapter and treats missing required Canonical Holon IR slots as validation diagnostics.
- **Closed-World Authoring Uniqueness** covers duplicate canonical symbols or keys, duplicate local property names, duplicate local relationship names, and duplicate inverse ownership in the current input model.
- **Blocking Semantic Diagnostics** include missing required slots, unresolved references, duplicate symbols, wrong descriptor or meta-kind, relationship inverse-pair violations, inheritance graph violations, TypeKind mismatches, effective-key failures, and generated-key mismatches.

## Example dialogue

> **Dev:** "When a user expands a relationship and applies a filter, are we just building a JSON query?"
> **Domain expert:** "No. We are building an **ExecutionPlan** from holon-backed **PlanSteps** so that navigation can be retrieved and replayed later."

## Flagged ambiguities

- "query expression" has been used to mean both the executable navigation/query structure and the returned query data. Resolved: use **ExecutionPlan**, **PlanNode**, and **PlanStep** for executable structure; use **Query Result** for returned data.
- Existing `Node`, `NodeCollection`, `QueryPathMap`, and `QueryExpression` names should stay unchanged while deprecated compatibility code remains. Resolved: do not rename them to `Legacy*`, do not extend them, and do not use them as foundations for new query/navigation design.
- `DanceRequest`, command wrappers, and future navigation envelopes are **Runtime Envelopes**, not **Runtime Shared Types**. Resolved: their disposition belongs in the corresponding surface/query docs, while `runtime-shared-types.md` governs carried runtime value/reference families.
- `QueryRequest`, `QuerySpec`, and `QueryResult` were old-world query envelopes. Resolved: PRO3 removes them rather than retaining or replacing them.
- Existing `TransactionAction::Query(QueryRequest)` was an unimplemented old-world command seam. Resolved: PRO3 removes it; future navigation should enter through descriptor-afforded Dances rather than a new transaction query action.
- Future navigation executable bodies are reference-first. Resolved: execute **ExecutionPlanReference** only after descriptor-backed navigation work introduces that contract; inline plan DTO execution is deferred.
- **ExecutionPlanReference** is a role-signaling facade in PRO3. Resolved: descriptor conformance validation is deferred.
- Query/plan/expand validation is deferred for Issue 508. Resolved: PRO3 may identify validation touchpoints, but does not enforce descriptor-backed structural validation.
- Future descriptor-backed navigation Dances are read-only from the command lifecycle perspective. Resolved: navigation execution may allocate **Transient Execution Artifacts** without becoming an undoable or staged mutation.
- Projection result shaping belongs to future **ProjectStep** work. Resolved: non-Project navigation behavior must not independently return row-shaped projections.
- "Query Command" was ambiguous between a Commands-layer action and an algebra operation. Resolved: avoid query command ingress; use descriptor-afforded Dances for navigation behavior and **PlanStep** or **Algebra Operation** for `Project`, `Expand`, `Filter`, and related query algebra steps.
- Non-project navigation query results are selected explicitly. Resolved: **ExecutionPlan** carries an **Output Binding**; **NavigationBindingSet** remains internal.
- **BoundHolonCollection** is the typed facade over its backing **HolonReference**. Resolved: do not introduce a separate `BoundHolonCollectionReference` name for PRO3.
- **BoundHolonCollection** member access follows the reference-layer relationship pattern. Resolved: expose a **Members Collection** handle and let callers use `HolonCollection` accessors rather than duplicating member-list convenience methods on the facade.
- **ItemType** is optional in PRO3. Resolved: do not require or enforce member conformance to item type in Issue 508.
- `VariableName` is not part of **BoundHolonCollection**. Resolved: variable identity is carried by plan steps and **NavigationBindingSet**.
- **NavigationBindingSet** is not a loose symbol table. Resolved: it carries variable bindings plus **NavigationTopology** for provenance/correlation, even if topology starts minimal in PRO3.
- **NavigationBindingSet** is in-memory execution state in PRO3. Resolved: do not represent it as a holon-backed/transient holon in Issue 508.
- **ExecutionPlan**, **PlanNode**, **PlanStep**, and specific step subtypes are holon-backed facades in PRO3. Resolved: do not hide plan internals as inline DTOs inside a plan holon.
- Pipeline ordering is relationship order in PRO3. Resolved: ordered **Pipeline Steps** use `HolonCollection` member order rather than per-step index properties or linked-list relationships.
- Minimal plan shape is holon-native in PRO3. Resolved: **ExecutionPlan** has **Output Binding** and **RootNode**; a pipeline **PlanNode** has ordered **Pipeline Steps**; step kind is conveyed by step subtype descriptor/facade rather than a `PlanStepKind` property.
- **Step Parameters** follow MAP holon modeling. Resolved: scalar parameters are properties; holon-reference-valued parameters are relationships.
- "validation" in TDL R4 can refer to syntax, IR structural, schema-aware, or runtime-loader boundary checks. Resolved: use **Validation Layer** for the classification and keep source-adapter responsibilities separate from **Canonical Holon IR** semantics.
- Partial semantic diff over invalid input is deferred. Resolved: R4 `diff` requires diagnostic-free lowering on both sides and reports layered diagnostics instead of attempting recovery.
- "extensibility" in TDL R4 should not mean a blanket ban on extending property or relationship descriptors. Resolved: use **TypeKind-Compatible Inheritance** as the authoring validation rule.
- `instance_type_kind` can be authored or imported, but it should not override the descriptor's semantic declaration kind. Resolved: R4 uses **Projected TypeKind** for inheritance checks and reports contradictions when authored/imported `instance_type_kind` disagrees.
- Inverse metadata for declared relationships is not optional schema-authoring information. Resolved: use **Relationship Pair Completeness** and require exactly one inverse per declared relationship in R4.
- Relationship descriptor cardinality bounds are required meta-descriptor properties. Resolved: R4 should require both `min_cardinality` and `max_cardinality` for relationship descriptors and validate `min_cardinality <= max_cardinality`.
- Descriptor inheritance flattening is deferred for Issue 578. Resolved: Airtable does not appear to provide inherited-effective validation, so R4 should validate local/schema-authoring structure without adding a new **Descriptor Semantics Service**.
- Duplicate local property names and relationship names are authoring errors in R4; duplicate inherited effective members are deferred with inheritance flattening.
- Inheritance graph health remains in scope for Issue 578. Resolved: R4 validates single-parent `Extends`, acyclic `Extends` chains, resolved `Extends` targets, and **TypeKind-Compatible Inheritance** without flattening inherited members.
- Full meta-descriptor requiredness derivation is deferred for Issue 578. Resolved: R4 uses a fixed **Required Slot Table**, including at least one variant for enum descriptors.
- Effective key validation is in scope for Issue 578 as the only inheritance-flattening exception. Resolved: R4 should validate Airtable-equivalent **Effective Key Rule** resolution, including `Extends` preference, `DescribedBy`/type fallback, known key-rule kinds, required key-rule inputs, and generated-key mismatch when an authored key is present; this does not reopen general property, relationship, command, dance, or operator flattening.
- Required descriptor slots are validated after source-adapter lowering. Resolved: TDL may apply explicit syntax-level conveniences or defaults, but Canonical Holon IR validation must report missing required semantic slots rather than inventing adapter-specific defaults.
- Uniqueness validation is closed-world for Issue 578. Resolved: R4 flags duplicates within the TDL, JSON, or Canonical Holon IR model under validation, but does not check whether another persisted MAP schema already uses the same key or symbol.
- Issue 578 semantic validation failures are errors. Resolved: all scoped schema-semantic invalidity produces **Blocking Semantic Diagnostics**; warnings are reserved for compatibility aliases or non-canonical source-adapter observations that do not make the Canonical Holon IR invalid.
- Diagnostics should carry both **Validation Layer** and **Diagnostic Origin**. Resolved: R4 should add an explicit layer field for responsibility classification while preserving origin for source location, symbol, or element attribution.
- Compile/decompile fidelity is semantic for Issue 578. Resolved: R4 compares normalized **Canonical Holon IR** content, including descriptors, projected kinds, references, required slots, key-rule semantics, relationship pairs, cardinalities, and literal semantic values, while ignoring source formatting, source ordering, and equivalent source-format shorthand.
- Runtime-loader boundary validation is limited to **Runtime Loader Projectability**. Resolved: R4 should catch Canonical Holon IR facts that make projection to the existing loader/import shape impossible or malformed, without refactoring loader validation, changing Nursery/PVL semantics, or introducing a new runtime import path.
