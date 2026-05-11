---
name: map-issue-grounder
description: "Use this skill when the user wants a GitHub Enhancement Issue generated for a specific MAP or map-holons workplan task, track item, PR unit, or wave-plan unit. Trigger on requests to ground a workplan task into an issue, turn a roadmap/spec/implementation-plan item into a GitHub enhancement, or generate, refine, or review a repository-grounded enhancement issue for MAP implementation work. This skill performs Phase 4 repository-grounded issue generation: prompt for copied DevDocs roadmap/spec inputs, inspect the map-holons repository, use the repository's .github/ISSUE_TEMPLATE/enhancement.md template, and produce an implementation-ready Enhancement Issue grounded in existing code."
---

# MAP Issue Grounder

Generate repository-grounded GitHub Enhancement Issues for MAP implementation work. Treat this as Phase 4 of the MAP development workflow: translate a stabilized DevDocs specification and bounded implementation-plan PR unit into a GitHub issue grounded in the actual `map-holons` repository.

## Trigger phrases

This skill should activate for requests like:

- "ground this workplan task into an issue"
- "turn this workplan item into a GitHub enhancement"
- "generate an enhancement issue for this PR unit"
- "create a repo-grounded issue for this roadmap task"
- "refine this enhancement issue against the current repo"
- "review this enhancement issue against the codebase"

Prefer this skill when the user is asking for a GitHub enhancement issue tied to a specific workplan task, not for general issue triage or broad implementation planning.

## Core posture

Use DevDocs artifacts as architectural intent, but do not treat them as sufficient for implementation. Ground every issue in repository reality:

- existing modules, types, APIs, tests, naming conventions, and architectural boundaries
- current transaction, staging, versioning, validation, dance, command, query, and SDK patterns
- current gaps between DevDocs intent and implemented substrate
- the issue structure required by `.github/ISSUE_TEMPLATE/enhancement.md`

Do not invent repository structures. If code context is missing, explicitly mark assumptions and ask for the relevant files or paths. Prefer extending existing patterns over introducing new abstractions.

## Required source artifacts

Before generating the issue, ensure the working context includes these artifacts in the `map-holons` repository:

1. The latest overall workplan copied from `map-dev-docs`:
   - source: `roadmap/desc-driven-impl-plan.md`
   - destination: `map-holons/docs/roadmap/desc-driven-impl-plan.md`
2. The target PR selection from that workplan:
   - track name or identifier
   - PR/unit identifier or title
3. The latest track-specific design spec copied from `map-dev-docs` into an appropriate `map-holons/docs/...` location.
4. The latest track-specific implementation plan copied from `map-dev-docs` into an appropriate `map-holons/docs/...` location.
5. The enhancement issue template from the code repository:
   - `.github/ISSUE_TEMPLATE/enhancement.md`

If any required artifact is missing, prompt the user to copy or identify it before continuing. Do not substitute a generic issue format for the enhancement template.

## Workflow

### 1. Prompt for the latest overall workplan

Ask the user to copy the latest overall workplan from `map-dev-docs` into `map-holons`:

- Copy `map-dev-docs/roadmap/desc-driven-impl-plan.md`
- To `map-holons/docs/roadmap/desc-driven-impl-plan.md`

Then inspect `map-holons/docs/roadmap/desc-driven-impl-plan.md`.

Use this file to understand:

- available tracks
- PR/unit identifiers
- wave sequencing
- dependency ordering
- prerequisites and downstream dependents
- the intended PR boundary for the target issue

### 2. Prompt for target track and PR

Ask the user to specify the target workplan unit:

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

### 3. Prompt for latest track-specific spec and implementation plan

Based on the selected track, ask the user to copy the latest relevant DevDocs files into the `map-holons` repository.

Ask for both:

- the latest design/specification file for the selected track
- the latest implementation-plan file for the selected track

The exact destination may vary by repository convention, but prefer a location under `map-holons/docs/` that preserves enough path context to identify the source and track, such as:

- `docs/specs/<track>/...`
- `docs/roadmap/<track>/...`
- `docs/devdocs-imports/<track>/...`

After the user copies the files, inspect them and identify their paths in the issue's source/context sections according to the enhancement template.

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

- what the copied DevDocs roadmap says this PR should do
- what the copied track spec says should exist
- what the copied implementation plan expects
- what the `map-holons` codebase already supports
- what must be added now
- what should be deferred
- what DevDocs may need to clarify
