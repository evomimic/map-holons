---
name: tdl-normative
description: Author, review, compile, or refactor MAP Schema 2.0 TDL against the authoritative TDL specification and descriptor-kernel semantics. Use for schema package boundaries, relationship declarations, generated imports, and TDL compiler changes.
---

# MAP TDL normative rules

Read these authoritative documents before changing TDL or its lowering:

- `map-dev-docs/docs/core/type-system/tdl/tdl-spec.md`
- `map-dev-docs/docs/core/type-system/descriptor-semantics-rules.md`

They override this checklist. This repository's `schema-src/` is authoritative source; regenerate `generated/json-imports/` and never hand-edit it.

## Authoring rules

- A file begins with one `schema <key>` declaration. `depends_on` is an exact, direct schema dependency; every cross-schema reference needs it. Schema dependencies are acyclic.
- Every non-schema declaration has exactly one explicit `type <key>`; it lowers to `DescribedBy`. Never infer it from declaration form or name.
- `extends` is optional, singular, and lowers only when authored. Never infer a parent, descriptor category, meta-type, or instance kind from names or syntax.
- Descriptor declarations contributed by the file imply `ComponentOf` that file's schema. `instance` does not.
- Use braced, newline-oriented `relationships` maps. A map entry is `Name -> target` or `Name -> [targets]`; each target is a key reference. A qualified relationship key is a reference to an existing declared descriptor, never an inline redefinition.
- `InstanceProperties` targets property descriptors. `InstanceRelationships` targets **declared** relationship descriptors only—never inverse descriptors.
- A declared relationship has exactly one `source`, `target`, `HasInverse`, and directional `deletion_semantic`; author the pairing only on its declared side. The inverse declares its own `source`, `target`, cardinality, and deletion semantic; do not author `InverseOf`/`inverse` on it.
- `abstract`, `def`, `ordered`, `duplicates`, and openness flags only lower to explicit `true`; omission is omission. `DefinesInstanceTypeKind true` is an ordinary local property, never implied by `abstract`.
- Preserve authored values and omissions. TDL lowering must not resolve references, bind contracts, materialize defaults, infer inverse occurrences, execute validation rules, or synthesize legacy `TypeKind`/runtime projections.

## Package-boundary rules

- A schema owns every descriptor holon contributed by its files, including both sides of relationship descriptors it declares.
- A dependent schema may author occurrences from its own holons to Core/package holons through a declared relationship in its own effective contract. Do not redeclare the external target holon to attach an edge.
- The declared relationship occurrence is authoritative. Commit materializes its inverse occurrence for traversal; do not reverse effective values virtually in runtime code to compensate for source layout.

## Required workflow

1. Identify authored loader facts to add/change: holon keys, `type`, properties, relationships, and package ownership.
2. Check every declaration against its source schema's direct dependencies and the target schema's package ownership.
3. Check relationship direction against the consumer API: the direct occurrence must be authored on the declared descriptor's source; inverse traversal depends on commit materialization.
4. Compile and inspect the generated JSON: each intended relationship must appear in the holon's `relationships` array, never as a property string.
5. Regenerate then run:

   ```sh
   npm run map-schema:compile:coreschema
   npm run map-schema:check:coreschema
   cargo test --manifest-path tools/map-schema/Cargo.toml --lib
   ```

6. For loader/runtime changes, add or retain a test that asserts the committed direct and inverse occurrences at the consuming API seam.

## Review stop conditions

Stop and correct the design if any change:

- relies on a transitive schema dependency;
- represents an inverse descriptor as an `InstanceRelationships` member;
- places a relationship descriptor's declared and inverse sides in different packages;
- treats a compact/inline form unsupported by the normative grammar as valid;
- converts an unrecognised TDL construct into a literal property instead of rejecting it; or
- moves descriptor-semantic inference, defaulting, or validation into the TDL compiler.
