# MAP Holons

This context defines the shared language for MAP holon runtime, query, navigation, dance, and command work in this repository.

## Language

**HolonReference**:
The canonical singular bound runtime handle for a holon within a transaction.
_Avoid_: Node id, raw holon pointer

**BoundHolonCollection**:
The canonical plural bound runtime result that carries a named collection of holon references without forcing row projection.
_Avoid_: RowSet, HolonCollection as a cross-surface contract

**BoundHolonCollectionReference**:
A typed Rust facade over a HolonReference pointing at a holon-backed BoundHolonCollection.
_Avoid_: Raw HolonCollection in new-world query contracts

**ExecutionPlan**:
A replayable MAP navigation/query view represented as holon-backed algebra operation nodes and accessed through typed Rust facades.
_Avoid_: Plain query DTO, row pipeline

**New-World Query Contract**:
The descriptor-aware, bound-first query/navigation contract built around HolonReference, BoundHolonCollection, and holon-backed ExecutionPlans.
_Avoid_: Legacy query bridge

**NavigationQueryRequest**:
The new-world transaction-bound request for executing MAP navigation/query work.
_Avoid_: QueryRequest when old-world compatibility remains ambiguous

**NavigationQuerySpec**:
The new-world request discriminator for navigation/query execution modes.
_Avoid_: QuerySpec when old-world compatibility remains ambiguous

**NavigationQueryResult**:
The new-world query result envelope that may return bound-first or materialized projection results.
_Avoid_: NodeCollection

**NavigationBindingSet**:
The internal bound-first closure object for new-world navigation/query execution, carrying named holon bindings, bound collections, and topology/provenance needed for composition.
_Avoid_: RowSet, a loose symbol table only

**ExecutionPlanReference**:
A typed Rust facade over a HolonReference pointing at a holon-backed ExecutionPlan.
_Avoid_: Raw HolonReference when the plan role matters

**Old-World Query Types**:
Deprecated compatibility types retained only to avoid breaking existing tests and transitional consumers.
_Avoid_: New query design foundation, Legacy-prefixed renames

**Spec Revision Session**:
A coherent design-update session that batches fine-grained decisions before applying one version bump per affected source spec.
_Avoid_: Per-decision spec version bumps

**PlanNode**:
A holon-backed structural node in an ExecutionPlan that organizes one or more plan steps.
_Avoid_: Result node, graph node

**PlanStep**:
A holon-backed navigation/query operation such as seed, expand, filter, project, distinct, order, skip, or limit.
_Avoid_: Command action, dance step

**Bound-First Operation**:
A navigation/query operation that consumes and produces HolonReference or BoundHolonCollection values rather than row-shaped projections.
_Avoid_: Row-native operator

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
- A **NavigationQuerySpec** may execute an **ExecutionPlanReference**.
- An **ExecutionPlan** contains one or more **PlanNodes**.
- A **PlanNode** contains one or more **PlanSteps**.
- Most **PlanSteps** are **Bound-First Operations**.
- Most **PlanSteps** consume and produce a **NavigationBindingSet**.
- A **NavigationBindingSet** is query-internal in PRO3 and is not a Commands, Dances, or SDK result contract.
- A **BoundHolonCollection** is a real holon-backed contract type, even if implementation helpers reuse existing HolonCollection mechanics internally.
- An **Expand** step extends a **NavigationBindingSet** with target **BoundHolonCollection** bindings and any topology/provenance needed for later composition.
- A **Filter** step consumes a **NavigationBindingSet** and produces a filtered **NavigationBindingSet**.
- A **Filter** step contains one or more **FilterExpressions** combined by exactly one **BooleanConnective** in PRO3.
- **Distinct**, **OrderBy**, **Skip**, and **Limit** should preserve **NavigationBindingSet** as their carrier in PRO3.
- A **ProjectStep** is the default operation that converts a **NavigationBindingSet** into a **Materialized Projection**.
- A **NavigationQueryResult** may preserve bound holon state or return a **Materialized Projection**.
- **Old-World Query Types** may remain for compatibility, but **New-World Query Contract** design must not depend on them.
- A **Spec Revision Session** closes when the team produces a stable artifact for one coherent design slice, such as a revised issue body.

## Example dialogue

> **Dev:** "When a user expands a relationship and applies a filter, are we just building a JSON query?"
> **Domain expert:** "No. We are building an **ExecutionPlan** from holon-backed **PlanSteps** so that navigation can be retrieved and replayed later."

## Flagged ambiguities

- "query expression" has been used to mean both the executable navigation/query structure and the returned query data. Resolved: use **ExecutionPlan**, **PlanNode**, and **PlanStep** for executable structure; use **Query Result** for returned data.
- Existing `Node`, `NodeCollection`, `QueryPathMap`, and `QueryExpression` names should stay unchanged while deprecated compatibility code remains. Resolved: do not rename them to `Legacy*`, do not extend them, and do not use them as foundations for new query/navigation design.
