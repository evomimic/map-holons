---
name: map-pr-review
description: "Use this skill when reviewing a map-holons GitHub pull request for merge readiness against one or more linked GitHub Issues. Trigger when the user asks to review a PR, assess whether a PR is merge-ready, compare a PR to issue specs, prepare review findings, or generate an approve comment for a map-holons PR."
---

# MAP PR Review

Review a `map-holons` pull request for merge readiness against the GitHub Issue specs it resolves. Optimize for a practical maintainer review: collect PR and issue context, verify CI state, sync the local branch, prompt the user for their manual verification results, inspect the diff against the specs, and then either post review findings or generate an approve comment.

## Trigger Phrases

Use this skill for requests like:

- "Review PR 509"
- "Is this map-holons PR merge-ready?"
- "Review this branch against issue 502"
- "Generate an approve comment for this PR"
- "Compare this PR to the issue spec and CI"

## Inputs

Accept any of these forms:

- PR number
- PR URL
- branch name
- issue number plus branch name
- pasted PR description or issue spec when GitHub access is unavailable

If the PR number is missing and cannot be inferred from the current branch, ask for it.

## Review Workflow

### 1. Collect PR Context

Use GitHub CLI when available:

```bash
gh pr view <pr> --repo evomimic/map-holons --json number,title,body,comments,headRefName,baseRefName,author,mergeStateStatus,reviewDecision,closingIssuesReferences,commits,url
```

Extract:

- PR title, description, author, branch, base branch, and current head SHA
- all PR comments relevant to scope, requested changes, follow-ups, or reviewer concerns
- issues GitHub says will close when merged
- any additional issue references in the PR body, title, or comments

If `closingIssuesReferences` is empty, search the PR body for phrases such as `Closes #`, `Fixes #`, `Resolves #`, `Issue #`, and direct issue URLs.

### 2. Check CI

Check the PR's current CI state:

```bash
gh pr checks <pr> --repo evomimic/map-holons
```

Rules:

- Treat required checks as merge blockers until they pass.
- If a check failed, inspect the failed job logs before deciding whether it is a code failure.
- If the failure is clearly GitHub runner infrastructure, caching, network, or disk exhaustion, report that distinction and rerun only with user approval.
- Do not call the PR merge-ready until CI has passed on the latest PR head SHA.

### 3. Load Issue Specs And Comments

For each issue identified from the PR:

```bash
gh issue view <issue> --repo evomimic/map-holons --comments --json number,title,body,comments,labels,state,url
```

Extract:

- summary and problem statement
- proposed solution
- explicit scope and out-of-scope items
- acceptance criteria or definition of done
- testing expectations
- any clarifying comments added after the issue body

Use the issue body and comments as the review contract. If the PR description and issue disagree, flag the discrepancy.

### 4. Sync Local Branch

Ensure the local checkout is reviewing the PR branch and is current with origin:

```bash
git fetch origin
git switch <headRefName>
git pull --ff-only
```

If the branch does not exist locally, create it from origin:

```bash
git switch -c <headRefName> --track origin/<headRefName>
```

Then confirm:

```bash
git status --short
git rev-parse HEAD
```

Rules:

- Never discard local changes.
- If the worktree has unrelated local changes, ignore them unless they affect the review.
- If local changes block checkout or update, explain the blocker and ask the user how to proceed.
- Compare the local HEAD SHA to the PR head SHA before reviewing.

### 5. Prompt For Manual Verification

Before final merge-readiness judgment, ask the user to report their manual results for:

- `build:happ`
- `build:host`
- `npm test`
- `npm start`

Do not run these commands yourself unless the user explicitly asks. Record the user's reported results in the final review assessment.

### 6. Inspect The Diff

Compare the PR branch to its base branch:

```bash
git diff --stat origin/<baseRefName>...HEAD
git diff --name-status origin/<baseRefName>...HEAD
```

Read changed files in the areas touched by the diff. Use `rg` for targeted searches and follow existing code paths far enough to understand behavior, tests, fixtures, and public surfaces affected by the change.

Specifically check:

- whether the PR adds new directories, crates, modules, or architectural layers not called out in the issue
- whether every required feature or acceptance criterion is implemented
- whether any scope creep appears outside the issue boundary
- whether logic, API signatures, serialization tags, fixtures, tests, and docs remain consistent
- whether tests cover the changed behavior at a risk-appropriate level
- whether the implementation follows existing repository patterns and naming conventions

For command, descriptor, schema, SDK, fixture, and sweettest changes, audit stringly referenced names with `rg`; do not rely only on compiler-renamed Rust symbols.

## Merge-Readiness Decision

Answer these questions explicitly:

1. Does the PR conform to the linked issue specs and comments?
2. Does it introduce major restructuring not called out in the issue?
3. Is any required functionality missing, or is there scope creep?
4. Is the code free of logic errors and behavioral regressions?
5. Is the code well structured and consistent with map-holons coding style?
6. Have CI checks passed on the latest PR head?
7. Did the user report successful manual verification for `build:happ`, `build:host`, `npm test`, and `npm start`?

If any answer is materially negative, the PR is not merge-ready.

## Not Merge-Ready Output

When the PR is not merge-ready, add a PR comment headed exactly:

```markdown
## Review Findings
```

Lead with findings, ordered by severity. Use file and line references when possible. Include:

- the issue/spec requirement violated
- the observed implementation problem
- the risk or user-visible consequence
- the smallest practical fix direction

Keep summary secondary. Do not bury blocking findings under praise.

Post with GitHub CLI when authorized:

```bash
gh pr comment <pr> --repo evomimic/map-holons --body-file <review-findings-file>
```

If posting is unavailable, generate copy-paste-ready markdown instead and tell the user it was not posted.

## Merge-Ready Output

When the PR is merge-ready:

- do not post directly unless the user explicitly asks
- use the `gen-md` skill to generate a single copy-paste-safe markdown block for an `Approve` PR comment
- mention linked issues, CI status, and the user's reported manual verification
- mention any non-blocking polish or follow-ups separately from merge-readiness
- include why the PR satisfies the issue contract and why any added follow-up commits or PR-description edits were appropriate

The approve comment should be concise and review-shaped, not a release note.

## Review Posture

Default to a code-review stance:

- findings first when there are problems
- no speculative blockers without repository evidence
- distinguish code failures from runner infrastructure failures
- distinguish issue-required behavior from nice-to-have follow-up work
- do not request compatibility shims, migrations, or scope expansion unless the issue or code evidence requires them
- prefer merge-ready with clear non-blocking polish over overfitting a PR to unrelated future work
