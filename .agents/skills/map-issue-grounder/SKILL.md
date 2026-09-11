---
name: map-issue-grounder
description: "Use this skill for initial authoring of a repository-grounded GitHub Enhancement Issue for a specific MAP or map-holons workplan task, track item, PR unit, or wave-plan unit. Trigger when the user wants to ground a workplan task into an issue, turn a roadmap/spec/implementation-plan item into a GitHub enhancement, or generate an implementation-ready Enhancement Issue grounded in current DevDocs and repository reality. Do not trigger for ordinary review, refinement, clarification, or minor updates to an existing issue definition unless the user explicitly requests a fresh repository-grounding pass. Load this skill at most once per conversation/session; reuse the grounding context already gathered rather than loading the skill and its source artifacts again."
---

# MAP Issue Grounder

Generate repository-grounded GitHub Enhancement Issues for MAP implementation work. Treat this as Phase 4 of the MAP development workflow: translate a stabilized DevDocs specification and bounded implementation-plan PR unit into a GitHub issue grounded in the actual `map-holons` repository.

## Applicability guard

This skill is for the **initial repository-grounding pass** used to author an implementation-ready issue from a workplan unit.

If the current request is only to review, refine, clarify, or make a minor update to an issue that has already been grounded, **stop here and handle the request directly without loading the source artifacts below**.

If this skill has already been applied during the current conversation/session, **do not load or execute it again**. Reuse the skill guidance and grounding context already gathered. Inspect additional or previously read sources only when the current request requires information not already available, the relevant source material may have changed, or the user explicitly requests fresh repository grounding.

## Core posture

Use `map-dev-docs` in situ as the authoritative source for architectural intent. Do not copy DevDocs specs, plans, or roadmaps into `map-holons`, and do not treat any copied local version as authoritative. Ground every initially authored issue in repository reality:

- existing modules, types, APIs, tests, naming conventions, and architectural boundaries
- current transaction, staging, versioning, validation, dance, command, query, and SDK patterns
- current gaps between DevDocs intent and implemented substrate
- the issue structure required by `.github/ISSUE_TEMPLATE/enhancement.md`

Do not invent repository structures. If code context is missing, explicitly mark assumptions and ask for the relevant files or paths when they are necessary to complete the grounding. Prefer extending existing patterns over introducing new abstractions.

## Required source artifacts

For the initial grounding pass, inspect these artifacts in the authoritative `map-dev-docs` checkout and the code repository:

1. The latest overall workplan in `map-dev-docs`:
   - `docs/roadmap/desc-driven-impl-plan.md`
2. The target PR selection from that workplan:
   - track name or identifier
   - PR/unit identifier or title
3. The latest track-specific design spec in `map-dev-docs`.
4. The latest track-specific implementation plan in `map-dev-docs`.
5. The enhancement issue template from the code repository:
   - `.github/ISSUE_TEMPLATE/enhancement.md`

These are requirements for **initial issue grounding**, not for every subsequent review or refinement of the resulting issue.

If the `map-dev-docs` checkout or a required artifact cannot be located, ask the user to identify its path or provide its content before continuing. Do not substitute a generic issue format for the enhancement template.

## Workflow

### 1. Inspect the latest overall workplan in situ

Locate and inspect `map-dev-docs/docs/roadmap/desc-driven-impl-plan.md` in the authoritative DevDocs checkout.

Use this file to understand:

- available tracks
- PR/unit identifiers
- wave sequencing
- dependency ordering
- prerequisites and downstream dependents
- the intended PR boundary for the target issue

### 2. Identify the target track and PR

Determine the target workplan unit from the user's request and available context.

If it is not already clear, ask the user to specify:

- track name or identifier
- PR/unit identifier or title

Then extract from the workplan:

- PR purpose
- scope boundary
- dependencies
- expected deliverables
- sequencing/wave context
- any stated non-goals

Preserve the PR boundary. Do not silently expand scope beyond the selected workplan unit.

### 3. Inspect the latest track-specific spec and implementation plan in situ

Based on the selected track, locate and inspect the latest relevant DevDocs design/specification and implementation-plan files in `map-dev-docs`.

Record their authoritative paths in the issue's source/context sections according to the enhancement template. If either cannot be located, ask the user to identify it; do not request a copy into `map-holons`.

### 4. Load the enhancement issue template

Read `.github/ISSUE_TEMPLATE/enhancement.md` from `map-holons`.

Use its headings, field names, ordering, checkboxes, comments, and expected structure as the output format.

Do not embed or rely on a default issue skeleton from this skill. The repository template is authoritative.

If the template contains HTML comments, preserve or adapt them only if they are useful in the final issue. Remove instructional comments when producing a clean issue body unless repository convention expects them to remain.

### 5. Restate design intent

Using the selected PR, spec, and implementation plan:

- summarize the capability or architectural change requested
- identify relevant MAP concepts, such as Holons, Dances, Commands, Queries, Agent Spaces, Trust Channels, membranes, transactions, validation, or SDK surfaces
- separate conceptual intent from implementation strategy
- identify constraints, invariants, and non-goals stated in DevDocs

### 6. Inspect repository reality

Inspect the actual `map-holons` codebase for adjacent or affected implementation areas.

Identify:

- relevant modules, structs/classes, traits/interfaces, and functions
- existing tests and fixtures
- current API/SDK surfaces
- related transaction, validation, query, dance, command, or type-system patterns
- naming conventions and architectural boundaries
- conflicts or gaps between DevDocs and current implementation

Record only repository-grounded findings. If something is inferred rather than observed, mark it as an assumption.

### 7. Map the PR intent to implementation substrate

Translate the selected PR into concrete repository work:

- affected files/modules if known
- data structure changes
- relationship or descriptor changes
- validation changes
- runtime behavior changes
- SDK/API changes
- test additions or updates
- migration or compatibility considerations

Distinguish:

- required work for this issue
- optional enhancements
- deferred follow-up work
- out-of-scope items

### 8. Surface questions and reconciliation notes

Identify unresolved questions that materially affect implementation.

Categorize them as:

- blocking
- non-blocking
- follow-up

Also identify DevDocs reconciliation notes:

- spec clarifications suggested by repository findings
- implementation assumptions that DevDocs should validate
- divergences between design intent and code reality
- terminology drift or naming inconsistencies

Do not hide design uncertainty inside vague acceptance criteria.

### 9. Generate the Enhancement Issue

Produce a complete issue body using `.github/ISSUE_TEMPLATE/enhancement.md` as the structure.

The issue must be:

- scoped to the selected track + PR unit
- grounded in current repository reality
- clear enough for a coding agent to create an implementation plan
- explicit about source documents inspected
- explicit about dependencies and sequencing
- testable through concrete acceptance criteria

Do not include code unless the user asks for implementation details in the issue.

After the initial issue has been grounded, subsequent review and refinement should normally be handled directly without invoking this skill or repeating the complete grounding workflow.

## Quality rules

- Use the repository's enhancement template, not a generic format.
- Preserve MAP terminology exactly when terms appear canonical.
- Flag possible terminology drift rather than silently normalizing it.
- Make acceptance criteria observable and testable.
- Avoid vague criteria such as "works correctly" or "handles all cases".
- Keep scope to one coherent PR unless the selected workplan unit is already too large.
- Separate must-implement-now from should-defer.
- Prefer current repository patterns over new abstractions.
- Record assumptions when repository evidence is incomplete.
- Reuse sufficient context already gathered rather than unnecessarily reloading sources.

## Split recommendation rule

Recommend splitting the selected PR/unit only when repository grounding shows it is not a coherent single issue. Split when any of these are true:

- it changes unrelated subsystems
- it requires more than one independent migration path
- it mixes foundational infrastructure with product-facing convenience APIs
- tests would need unrelated fixtures or harnesses
- acceptance criteria cannot be made coherent under one PR

When recommending a split, provide the smallest coherent sequence of issues and explain dependencies, but still generate the best bounded issue possible for the requested unit.

## Reconciliation emphasis

The highest-value output is not a generic issue. The highest-value output records the discovered fit between:

- what the authoritative DevDocs roadmap says this PR should do
- what the authoritative track spec says should exist
- what the authoritative implementation plan expects
- what the `map-holons` codebase already supports
- what must be added now
- what should be deferred
- what DevDocs may need to clarify