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
The descriptor-aware, bound-first query/navigation contract built around HolonReference, BoundHolonCollection, and holon-backed ExecutionPlans.
_Avoid_: Legacy query bridge

**Runtime Shared Type**:
A canonical value or reference family reused across MAP surfaces without owning a surface's request or response envelope.
_Avoid_: Surface envelope, command wrapper

**Runtime Envelope**:
A surface-owned request or response container for commands, dances, queries, or trust-channel transport.
_Avoid_: Runtime shared type

**NavigationQueryRequest**:
The new-world transaction-bound request for executing MAP navigation/query work.
_Avoid_: QueryRequest when old-world compatibility remains ambiguous

**NavigationQuerySpec**:
The new-world request discriminator for navigation/query execution modes.
_Avoid_: QuerySpec when old-world compatibility remains ambiguous

**NavigationQueryResult**:
The new-world query result envelope whose shape is determined by the executed navigation algebra, with projections produced only by Project.
_Avoid_: NodeCollection

**NavigationQuery Command Action**:
The new-world transaction command action that adapts command ingress into the navigation query substrate.
_Avoid_: Reusing old-world Query action

**Transient Execution Artifact**:
A transaction-scoped transient holon created during read-only navigation query execution to preserve bound-first intermediate or result state.
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

**Old-World Query Types**:
Deprecated compatibility types retained only to avoid breaking existing tests and transitional consumers.
_Avoid_: New query design foundation, Legacy-prefixed renames

**QueryRequest**:
The retained old-world query envelope used for compatibility with transitional query/navigation behavior.
_Avoid_: New-world navigation query envelope

**QuerySpec**:
The retained old-world query body discriminator used by the compatibility query envelope.
_Avoid_: NavigationQuerySpec

**QueryResult**:
The retained old-world query result envelope used by compatibility query pathways.
_Avoid_: NavigationQueryResult

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
The outcome of query execution, which may be bound-first through HolonReference or BoundHolonCollection, or materialized as BaseValue, Row, or RowSet at projection boundaries.
_Avoid_: Query expression

**Materialized Projection**:
A scalar, row, or rowset shape produced when a contract, projection, ordering, distinctness, pagination, ABI, or serialization boundary requires values.
_Avoid_: Internal execution state

**ProjectStep**:
The default PRO3 materialization boundary that converts bound query state into BaseValue, Row, or RowSet.
_Avoid_: Implicit row materialization by order, distinct, skip, or limit

## Relationships

- A **NavigationQueryRequest** contains a **NavigationQuerySpec**.
- In PRO3, a **NavigationQuerySpec** executes an **ExecutionPlanReference** rather than carrying an inline plan DTO.
- **QueryRequest**, **QuerySpec**, and **QueryResult** remain old-world compatibility envelopes.
- **NavigationQueryRequest**, **NavigationQuerySpec**, and **NavigationQueryResult** are the new-world query/navigation envelopes for PRO3.
- A **NavigationQuery Command Action** carries a **NavigationQueryRequest**.
- A **NavigationQuery Command Action** is command-read-only while still allowing **Transient Execution Artifacts**.
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
- A **ProjectStep** is the default operation that converts a **NavigationBindingSet** into a **Materialized Projection**.
- A **NavigationQueryResult** returns a **Materialized Projection** only when the executed algebra reaches **ProjectStep**.
- Without **ProjectStep**, a **NavigationQueryResult** returns the bound value selected by the **Output Binding**.
- **Old-World Query Types** may remain for compatibility, but **New-World Query Contract** design must not depend on them.
- A **Spec Revision Session** closes when the team produces a stable artifact for one coherent design slice, such as a revised issue body.

## Example dialogue

> **Dev:** "When a user expands a relationship and applies a filter, are we just building a JSON query?"
> **Domain expert:** "No. We are building an **ExecutionPlan** from holon-backed **PlanSteps** so that navigation can be retrieved and replayed later."

## Flagged ambiguities

- "query expression" has been used to mean both the executable navigation/query structure and the returned query data. Resolved: use **ExecutionPlan**, **PlanNode**, and **PlanStep** for executable structure; use **Query Result** for returned data.
- Existing `Node`, `NodeCollection`, `QueryPathMap`, and `QueryExpression` names should stay unchanged while deprecated compatibility code remains. Resolved: do not rename them to `Legacy*`, do not extend them, and do not use them as foundations for new query/navigation design.
- `QueryRequest`, `DanceRequest`, command wrappers, and query result envelopes are **Runtime Envelopes**, not **Runtime Shared Types**. Resolved: their disposition belongs in the corresponding surface/query docs, while `runtime-shared-types.md` governs carried runtime value/reference families.
- `QueryRequest`, `QuerySpec`, and `QueryResult` are old-world compatibility envelopes. Resolved: PRO3 introduces distinct **NavigationQueryRequest**, **NavigationQuerySpec**, and **NavigationQueryResult** envelopes instead of extending old-world query envelopes.
- Existing `TransactionAction::Query(QueryRequest)` remains old-world compatibility. Resolved: PRO3 adds a distinct **NavigationQuery Command Action** for new-world query/navigation execution.
- `NavigationQuerySpec` executable bodies are reference-first in PRO3. Resolved: execute **ExecutionPlanReference** only; inline plan DTO execution is deferred.
- **ExecutionPlanReference** is a role-signaling facade in PRO3. Resolved: descriptor conformance validation is deferred.
- Query/plan/expand validation is deferred for Issue 508. Resolved: PRO3 may identify validation touchpoints, but does not enforce descriptor-backed structural validation.
- **NavigationQuery Command Action** is read-only from the command lifecycle perspective. Resolved: query execution may allocate **Transient Execution Artifacts** without becoming an undoable or staged mutation.
- Projection result shaping belongs to **ProjectStep**. Resolved: non-Project query/navigation commands must not independently return `BaseValue`, `Row`, or `RowSet` projections.
- "Query Command" was ambiguous between a Commands-layer action and an algebra operation. Resolved: use **NavigationQuery Command Action** for Commands ingress and **PlanStep** or **Algebra Operation** for `Project`, `Expand`, `Filter`, and related query algebra steps.
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
