# AGENTS.md

## Project purpose

This repository includes repo-local Agent Skills that follow the Agent Skills standard. The skills live alongside the `map-holons` codebase so they can be versioned, reviewed, and evolved together with the implementation and design docs they depend on.

The repository copy of each skill is the canonical shared source for all contributors, including contributors using Codex or Claude Code. Tool-specific metadata may live inside a skill directory, but the skill's core instructions should remain as tool-neutral as possible.

## Repository layout

- `.agents/skills/<skill-name>/` — canonical repo-local skill directory.
- `.agents/skills/<skill-name>/SKILL.md` — required skill entrypoint.
- `.agents/skills/<skill-name>/agents/openai.yaml` — optional OpenAI/Codex-specific metadata.
- `.agents/skills/<skill-name>/references/` — optional supporting documentation.
- `.agents/skills/<skill-name>/scripts/` — optional executable helpers.
- `.agents/skills/<skill-name>/assets/` — optional templates, examples, or other static files.

## Tool usage expectations

- Keep the repo-local skill directory as the canonical shared definition.
- Write `SKILL.md` so the main workflow and constraints are useful to both Codex and Claude Code.
- Put tool-specific UI or metadata files under the skill directory without making the main instructions depend on them.
- Do not assume any tool will automatically discover repo-local skills from `.agents/skills/`.

For Codex specifically:

- repo-local skills may be used when explicitly referenced by name or path
- automatic first-class skill discovery generally requires installing or symlinking the skill into `~/.codex/skills/` and restarting Codex

For Claude Code specifically:

- keep the shared repo instructions readable and self-sufficient
- avoid putting essential workflow rules only in Codex-specific metadata files

## Skill authoring rules

- Use lowercase, kebab-case skill directory names.
- Every skill must include `SKILL.md`.
- Keep `SKILL.md` concise and operational.
- Put detailed background material in `references/`.
- Put deterministic or fragile repeatable logic in `scripts/`.
- Put reusable templates and static files in `assets/`.
- Do not include generated build artifacts, dependency folders, secrets, or large binary files in skills.
- Prefer examples that show exact expected inputs and outputs.

## `SKILL.md` requirements

Each `SKILL.md` must start with YAML frontmatter:

```yaml
---
name: example-skill
description: concise description of what the skill does and when to use it.
---
```

Rules:

- `name` must match the skill directory name.
- `name` must be lowercase kebab-case.
- `description` must explain when the skill should be used.
- The body should describe the workflow, constraints, and any resources to consult.

## Agent metadata

If present, `agents/openai.yaml` should contain OpenAI-specific UI metadata, for example:

```yaml
interface:
  display_name: Example Skill
  short_description: Helps with a repeatable task.
```

## Validation checklist

Before considering a skill complete:

- Confirm the skill has `SKILL.md`.
- Confirm frontmatter includes only `name` and `description`.
- Confirm the skill name is lowercase kebab-case.
- Confirm the description is specific enough to trigger the skill appropriately.
- Remove unused placeholder files.
- Run or inspect any scripts added under `scripts/`.
- Ensure no secrets, credentials, private keys, or large generated files are committed.

## Coding conventions

- Keep scripts small, readable, and deterministic.
- Prefer Python or shell scripts only when procedural reliability is needed.
- Include clear usage comments at the top of scripts.
- Fail loudly with actionable error messages.

## Git conventions

- Keep commits focused.
- Do not mix unrelated skill changes in one commit.
- When modifying a skill, update its supporting files and `SKILL.md` together if needed.
- Do not rewrite history unless explicitly asked.

## What not to do

- Do not place multiple unrelated skills in one skill directory.
- Do not put OpenAI metadata at the repository root.
- Do not duplicate long reference material inside `SKILL.md`.
- Do not make the core skill workflow understandable only to one tool when the repo skill is intended to be shared.
- Do not assume a skill is complete if it has not been checked against the validation checklist.
