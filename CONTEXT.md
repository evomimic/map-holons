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

**Relationship Endpoint Type**:
The `HolonType` descriptor that classifies holons permitted at the source or target of a declared relationship.
_Avoid_: Arbitrary `TypeDescriptor` endpoint, property or relationship descriptor endpoint

**Holon Ownership**:
The required, infrastructure-managed `OwnedBy` relationship from each holon to exactly one current `HolonSpace`.
_Avoid_: Optional ownership, multiple owning spaces, explicitly authored ownership

**Directional Deletion Semantic**:
The deletion behavior explicitly authored on each declared or inverse relationship descriptor for deletion of that descriptor's source holon.
_Avoid_: Deriving an inverse descriptor's deletion behavior from its declared partner

**Abstract Relationship Endpoint**:
An abstract type used as a polymorphic relationship constraint; each actual endpoint is a holon whose effective descriptor equals or transitively extends the declared endpoint type.
_Avoid_: Directly instantiating the abstract endpoint, rejecting abstract endpoint constraints

**Uniform Endpoint Compatibility**:
The rule that every relationship endpoint is validated as a holon through its effective semantic type and transitive `Extends`.
_Avoid_: Using meta-types as descriptor-to-descriptor endpoint categories

**EffectiveEndpointType**:
The semantic type used for endpoint validation: the holon itself when it is a type descriptor, otherwise its `DescribedBy` type.
_Avoid_: Meta-type substitution for descriptor endpoint classification, separate endpoint validators

**Meta-Type Holon Classification**:
The classification established by `MetaTypeDescriptor Extends HolonType`, making every concrete meta-type transitively substitutable for `HolonType`.
_Avoid_: A separate meta-type `Extends` hierarchy, special descriptor-holon endpoint rules

**Descriptor Endpoint Category**:
An abstract descriptor type such as `PropertyType`, `ValueType`, or `DeclaredRelationshipType` used to classify descriptor holons participating in descriptor-to-descriptor relationships.
_Avoid_: The meta-type that governs those descriptor holons' conformance

**RelationshipType**:
The abstract descriptor-classification root shared by `DeclaredRelationshipType` and `InverseRelationshipType`, and the polymorphic source constraint for `SourceType` and `TargetType`.
_Avoid_: `MetaRelationshipType` as a descriptor-to-descriptor endpoint, duplicated relationship key rules on declared and inverse roots

**MetaValueType**:
The concrete meta-type that describes value-type descriptor holons and extends `MetaTypeDescriptor`.
_Avoid_: Abstract describing meta-type, `ValueType Extends MetaValueType`

**Abstract Descriptor Completeness**:
The rule that an abstract descriptor may omit conformance-contract members with positive minimum cardinality, while all supplied values and universal structural invariants remain valid.
_Avoid_: Generic placeholder targets solely to satisfy abstract roots, exempting concrete descriptors

**Default Descriptor Key Rule**:
The provisional Schema 2.0 rule that `MetaTypeDescriptor` supplies `ExtendedTypeRule` as the default key rule for type-descriptor holons. A descriptor key combines its local `type_name` with the immediate extended descriptor's `type_name`; the sole descriptor without `Extends` falls back to its local `type_name`.
_Avoid_: Accumulating ancestor keys, using the parent's composed key, adding a meta-type solely to vary an extension family's descriptor-key suffix

**Holon Instance Key Baseline**:
The explicit `NoneRule.KeyRuleType` selected by root `HolonType`; instances of extension holon types remain keyless unless their describing type or a nearer ancestor overrides `InstanceKeyRule`.
_Avoid_: Confusing descriptor-holon keys with described-instance keys, treating an omitted effective key rule as keylessness

**InstanceKeyRule**:
The required override-inherited relationship from a holon type to the key rule governing holons described by that type. Its inverse is `KeyRuleForInstancesOf`.
_Avoid_: `UsesKeyRule`, implying that the rule governs the source type descriptor's own key, applying instance key rules to non-holon descriptor categories

**DescribedTypeRule**:
A key rule for named ordinary holons that composes the holon's local `type_name` with the `type_name` of its `DescribedBy` type.
_Avoid_: Applying `ExtendedTypeRule` to holons without `Extends`, using an unqualified name when the describing type is part of identity

**TDL Declaration Form**:
The syntactic form selected by a top-level TDL keyword. Descriptor-oriented forms such as `holon`, `property`, `value`, and `enum` provide type-authoring shorthand, while `instance` is the generic holon form. A declaration form does not infer `TypeKind` or `DescribedBy`; every non-schema declaration supplies an explicit `type` and its authored key must conform to that type's effective instance key rule. The specialized `schema` form establishes compilation scope and lowers to a schema holon.
_Avoid_: Declaration kind, treating `instance` as a TypeKind, inferring semantic classification from a TDL keyword

**TDL Key Reference**:
An authored holon key or reference written bare when it contains no whitespace or structural delimiters, and as a quoted string when it does. Quoting is lexical only and does not change key identity.
_Avoid_: Schema-specific whitespace parsing, treating quoted keys as property-value literals in key/reference positions

**TDL Corpus Protection Baseline**:
A transitional baseline that protects known corpus invariants while the current source tooling cannot yet semantically accept the full Schema 2.0 TDL corpus.
_Avoid_: Semantic acceptance milestone, full parser conformance

**Transitional Corpus Scanner**:
A test-private helper that recognizes only the TDL declaration blocks and clauses needed to guard the corpus before the real source tooling can parse the full Schema 2.0 corpus.
_Avoid_: TDL parser, semantic scanner, production source model

**LoaderRefRep**:
The existing schema-backed loader reference representation rooted at HolonLoadSet and composed of loader holons, loader relationship references, and loader holon references.
_Avoid_: New type system, semantic IR, tool-local loader IR

**MAP Source Syntax**:
An authoring syntax for expressing holons, including type descriptor holons, as LoaderRefRep-compatible facts; JSON and TDL are alternate concrete syntaxes for the same authored holon content.
_Avoid_: Treating generated JSON as an arbitrary compiler backend, giving TDL-only semantics to shared holon facts

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
- `SourceType` and `TargetType` target **Relationship Endpoint Types** rooted at `HolonType`, because MAP relationships connect holons rather than arbitrary descriptor kinds.
- **Holon Ownership** has cardinality `1..1`; its `Owns` inverse remains `0..32767`.
- `MetaRelationshipType` supplies `DeletionSemantic` to both declared and inverse relationship descriptor contracts; each direction explicitly supplies its own **Directional Deletion Semantic**.
- An **Abstract Relationship Endpoint** constrains actual endpoint holons through their effective descriptors and transitive `Extends`.
- **Meta-Type Holon Classification** permits meta-type descriptors to participate in the generalized `(HolonType)-[DescribedBy]->(TypeDescriptor)` relationship and its `(TypeDescriptor)-[Instances]->(HolonType)` inverse.
- **Uniform Endpoint Compatibility** applies because all descriptors are holons: endpoint conformance always resolves through the endpoint holon's effective semantic type.
- Endpoint validation applies one rule: `EffectiveEndpointType(H) Extends* requiredType`.
- Meta-types declare what descriptor holons must contain through `InstanceProperties` and `InstanceRelationships`; **Descriptor Endpoint Categories** define which semantic descriptor categories may participate as relationship endpoints.
- **RelationshipType** extends `TypeDescriptor`, supplies the shared relationship key rule, and is extended by both declared and inverse relationship descriptor roots.
- Core inverse relationships use explicit `Block` deletion semantics to preserve references and contracts, except `MemberOfCollection`, which uses `Allow` for non-owning collection membership.
- `InstanceProperties` and `InstanceRelationships` use `TypeDescriptor` as their semantic source endpoint; meta-types attach those authoritative relationship keys through their instance contracts.
- Every enum-variant descriptor has exactly one `VariantOf` owner; the inverse `Variants` relationship remains `0..32767`.
- `MetaValueType` is concrete because value-type descriptor holons are its direct instances; value descriptor classification separately follows `ValueType Extends TypeDescriptor`.
- `AffordsOperator` uses `InheritanceMode Additive`, allowing operators populated on abstract value categories to accumulate on concrete value-type descendants.
- `EnumVariantValueType` is described by concrete `MetaValueType` and extends `ValueType`; its variant declaration kind does not infer a separate meta-type.
- Relationships such as `AffordsOperator`, `Constraints`, `Variants`, and `ElementValueType` describe value descriptor holons; their permissions belong to value meta-type contracts rather than to the instance contracts passed to non-holon values.
- `MetaValueArrayValueType` extends `MetaValueType` and describes array value-type descriptors, keeping required `ElementValueType` and array-specific `Constraints` out of the common value-descriptor contract.
- `MetaStringValueType`, `MetaIntegerValueType`, `MetaBytesValueType`, `MetaEnumValueType`, and `MetaValueArrayValueType` provide kind-specific value-descriptor contracts without TypeKind inference or ambiguous `Constraints` members.
- **Abstract Descriptor Completeness** allows `PropertyType`, `EnumValueType`, and `ValueArrayValueType` roots to omit required concrete descriptor state such as `ValueType`, `Variants`, and `ElementValueType`.
- **Default Descriptor Key Rule** keeps descriptor keys two-part and makes reparenting a descriptor an identity change; a complete reference-impact audit is required before migrating existing authored keys.
- **Holon Instance Key Baseline** is inherited through **InstanceKeyRule** with `InheritanceMode Override`; a local key rule replaces rather than combines with the nearest inherited rule.
- Configured format rules are ordinary instances described by `FormatRule.KeyRuleType`; `ImplementationNameRule.FormatRule` is not retained as a descriptor subtype.
- `ImplementationName.FormatRule` is the configured format-rule instance for `DanceImplementation` keys; it uses template `{0}` with `ImplementationName.PropertyType` as its sole ordered `TemplateParameters` target. It is not an `ImplementationRule` type or descriptor.
- `FormatRule.KeyRuleType` uses **DescribedTypeRule** for configured instances and declares `TypeName`, `TemplateString`, and ordered `TemplateParameters` as their contract.
- `FormatRule.KeyRuleType` is concrete because configured format-rule holons use it as their `DescribedBy` target; only the common `KeyRuleType` classification root remains abstract.
- **InstanceKeyRule** has effective cardinality `1..1` and `InheritanceMode Override`; explicit `NoneRule.KeyRuleType` represents keylessness.
- Directional deletion permits deleting a holon-type descriptor through `InstanceKeyRule` (`Allow`) but blocks deleting a key rule while `KeyRuleForInstancesOf` references remain (`Block`).
- `NoneRule.KeyRuleType` is the canonical referenced descriptor key for explicit keylessness; `NoneKeyRule` is not a normative alias.
- TDL supports `instance` as its generic holon declaration form alongside descriptor-oriented forms; specialized declaration keywords remain shorthand and do not determine semantic `TypeKind`.
- TDL keys and references containing whitespace or structural delimiters are quoted; delimiter-free keys may remain bare.
- A complete TDL file uses the specialized `schema` declaration to establish its containing schema and the target of implicit descriptor `ComponentOf`; generic instances receive no implicit `ComponentOf`.
- `depends_on` is specialized compilation syntax because dependencies establish the resolution closure before ordinary holon validation; it lowers to the schema holon's semantic `DependsOn` relationship.
- A **TDL Corpus Protection Baseline** may use focused source-level guards for known corpus invariants, but it does not claim full TDL parser acceptance or descriptor-semantic validation.
- A **Transitional Corpus Scanner** may exist only in tests and may recognize just enough TDL block shape to protect R0 invariants before R6 replaces it with real source tooling.
- `map-schema:check:coreschema` remains the honest current-tooling acceptance check for the full corpus; R0 does not make that command pass by downgrading known parser failures.
- R0 corpus guards cover declared-side inverse-pair orientation, selected additive inheritance anchors, the `InstanceKeyRule` cardinality and override anchors, and the current explicit-`type` parser blocker.
- R0 corpus guards do not validate schema dependency DAGs, cross-schema reference coverage, descriptor conformance, abstract descriptor completeness, endpoint compatibility, default validity, enum semantics, or `TypeKind` migration policy.
- R0 corpus guard tests live near existing `tools/map-schema` corpus tests and isolate their **Transitional Corpus Scanner** in a clearly removable test module.
- R6 source tooling compiles file-to-file while remaining isomorphic to **LoaderRefRep** construction: `map-schema compile` renders loader import JSON, and the existing Holon Loader client/runtime path constructs the actual transient loader holon graph.
- R6 host diagnostics are limited to TDL syntax, source-to-loader-JSON lowering shape, and package-visible duplicate authored keys. Authored relationship targets are preserved as keys; dependency-closure reference resolution belongs to the existing loader reference resolver and later descriptor validation.
- R6 specialized TDL declaration forms are authoring notation only. Except for `schema` scope behavior, they lower like generic `instance` declarations and must not infer descriptor category, `type`, `Extends`, `DefinesInstanceTypeKind`, legacy `InstanceTypeKind`, or `TypeKind`.
- R6 preserves implicit `ComponentOf` as schema packaging syntax for descriptor-oriented declaration forms only. Generic `instance` declarations receive no implicit `ComponentOf`, and an explicit descriptor-authored `ComponentOf` in the same TDL file is rejected as ambiguous package ownership.
- R6 treats `extends` as shorthand for exactly one `Extends` relationship target. Omitted `extends` means no local `Extends`; a declaration that combines `extends` with an authored `Extends` relationship is rejected as duplicate structural authorship.
- R6 lowers TDL `type` to the loader JSON top-level `type` field so the existing loader client constructs `DescribedBy`. For fidelity, this is the same LoaderRefRep fact as `DescribedBy`; explicitly authoring both `type` and `DescribedBy` is rejected as duplicate describing authorship.
- R6 canonicalizes repeated ordinary relationship-map entries into one loader JSON relationship per relationship name while preserving authored target order. Duplicate ordinary names accumulate; duplicate shorthand authorship for `type`, `extends`, or implicit `ComponentOf` remains an error.
- R6 renders generated loader JSON relationship targets as canonical `$ref` objects. Local authored keys are preserved inside `$ref` without host-side resolution, leaving room for future saved or external reference forms without changing the relationship target shape.
- R6 does not change the existing loader behavior that treats leading `#` as non-semantic local-reference syntax. Generated JSON should omit the leading `#`; exact source spelling, when needed, belongs in provenance rather than key identity.
- R6 provenance is a source-agnostic, bounded sidecar keyed to loader facts so errors discovered anywhere in the chain can anchor back to the offending source span for highlighting. R6 keeps this sidecar internal/in-memory for diagnostics and tests; it does not add a persisted provenance artifact unless a later consumer requires one.
- R6 lowers presence-based Boolean shorthand only when authored, producing explicit `true` values. Absence remains omission; R6 does not emit `false` defaults or materialize descriptor-defined defaults.
- R6 accepts only full cardinality ranges of the form `min..max` or `min..*`. It lowers finite maxima to `MaxCardinality`, lowers `*` by omitting `MaxCardinality`, and never emits finite sentinel values for unbounded cardinality.
- R6 lowering must not depend on resolving a referenced descriptor and inspecting its contract. Supported syntax lowers mechanically to loader facts; descriptor binding, contract lookup, and semantic validation happen later in the loader and validator pipeline.
- R6-generated loader JSON uses Schema 2.0 authored member names such as `TypeName`, `IsAbstractType`, `MinCardinality`, and `DeletionSemantic`; it does not emit legacy snake_case adapter names or legacy `InstanceTypeKind`/`TypeKind` projections.
- JSON and TDL are alternate **MAP Source Syntax** forms for the same authored holon content. TDL may offer shorthand notation and rely on loader/default materialization, but generated JSON should not introduce an arbitrary dialect or representation differences unrelated to those shorthand choices.
- R6 should choose canonical JSON shapes that support R7 round-trip idempotency. After one normalization pass, `TDL -> JSON -> TDL` should produce the same canonical TDL and `JSON -> TDL -> JSON` should produce the same canonical JSON, backed by LoaderRefRep signature equality rather than original formatting or shorthand preservation.
- R6 canonical JSON renders every relationship target as an ordered array of `$ref` objects, including singleton relationships. The loader may continue accepting scalar targets and string refs as input compatibility, but generated/canonical JSON uses one uniform target shape.
- R6 canonical JSON preserves authored omissions. Presence-based Boolean shorthand emits explicit `true` only when authored; omitted booleans/defaultable values are not generated as `false`.
- R6 canonical JSON omits volatile generated metadata such as timestamps. Schema dependencies lower to the schema holon's `DependsOn` relationship; import orchestration metadata such as `load_with` is not the canonical source representation of Schema 2.0 dependency semantics.
- R6 canonical JSON uses explicit ordering tables for common properties and relationships with alphabetical fallback. Declaration/holon order follows deterministic source discovery and declaration order; relationship target order preserves authored order exactly.
- R6 rejects syntactic self-contradictions inside a relationship descriptor, such as a qualified relationship key whose source/name/target segments disagree with authored `source` or `target` clauses. This is exact source consistency, not endpoint substitutability or semantic compatibility validation.
- R6 requires exactly one `source` and one `target` clause on declared and inverse relationship declaration forms. Generic `instance` declarations do not admit relationship-descriptor shorthand.
- R6 inline enum variants lower to independent variant descriptor holons plus an enum-side `Variants` relationship on the enclosing enum. Canonical JSON does not author inverse-side `VariantOf`; the `tdl-spec.md` sentence saying inline variants lower to `VariantOf` is stale relative to the current corpus and declared-side authoring rule.
- Branch 625 should not directly edit `map-dev-docs` while the docs PR is under review; record spec inconsistencies as implementation notes or follow-ups unless a paired docs branch is explicitly requested.
- R6 acceptance means generated JSON passes the bootstrap import JSON schema and existing loader-client structural parsing into LoaderRefRep. Guest-side descriptor-default materialization, descriptor validation, and validated commit may remain downstream R3/R4/R5 work.
- R6 vertically replaces the TDL compile/check path so it no longer routes through `SemanticModel`. Existing decompile and legacy comparison paths may remain temporarily until R7/R8, but production TDL compile/check should use the R6 loader-JSON lowering path.
- After R6, `map-schema check` performs parse/lower/structural validation only: TDL parsing, canonical loader JSON lowering, bootstrap import JSON-schema validation, and any transaction-free parse-only loader JSON shape pass. It must not construct loader holons, create transactions, resolve references, stage holons, materialize defaults, invoke loader dances, or commit.
- R6 should prefer a transaction-free parse-only API owned by `holons_loader_client` for loader JSON shape validation, consumed by `tools/map-schema`, rather than duplicating the loader JSON parser in source tooling.
- R6 may add parse-only JSON validation primitives needed for structural acceptance, but JSON-to-TDL rendering, decompilation, and LoaderRefRep fidelity signatures remain R7 work.
- R6 converts the R0 expected parser-failure tests into green corpus acceptance tests. R0 source-visible guards remain until equivalent assertions can run against real R6 parsed/lowered facts, at which point the test-private Transitional Corpus Scanner should be deleted.

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
- "Executable baseline" was ambiguous for TDL R0. Resolved: use **TDL Corpus Protection Baseline** for the R0 posture until the source tooling can parse and lower the Schema 2.0 corpus through the intended representation.
- R0 corpus guards should not be implemented as a second source parser. Resolved: use a test-private **Transitional Corpus Scanner** with deliberately narrow scope, then delete or replace it when R6 parsing/lowering takes over.
- `map-schema:check:coreschema` should not be softened into a transitional guard command. Resolved: leave it failing until real parser support lands; R0 protection runs through focused tests.
- R0 guard scope was at risk of absorbing descriptor-kernel validation. Resolved: R0 guards only source-visible corpus anchors; graph and semantic validations wait for R6/R4 infrastructure.
- The 625 branch is stacked on 624 as a temporary integration strategy, not a durable architecture decision. Resolved: record this in the implementation plan/PR notes rather than creating an ADR.
- R0 test placement should not scatter transitional helpers. Resolved: keep the tests in `tools/map-schema/src/tdl_compiler.rs` near existing corpus tests, isolated in a removable nested module.
- Existing green core-corpus tests conflict with the current explicit-`type` parser blocker. Resolved: convert them to precise expected-failure tests instead of deleting or broadly ignoring them.
- R0 inventory should stay shallow and actionable. Resolved: group migration surfaces by owner, current role, R6 disposition, and blockers rather than auditing every call site.
