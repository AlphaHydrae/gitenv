---
name: gitenv-documentation-management
description: Keep gitenv's migration and agent documentation consistent, up-to-date, and coherent across files. Use when completing increments, updating guidelines, managing the migration backlog, or ensuring documentation stays in sync.
---

# Gitenv Documentation Management

This skill guides the process of keeping gitenv's migration and agent
documentation consistent, up-to-date, and coherent across files.

## When to use this skill

- After completing a Rust migration increment
- When migration scope, sequencing, assumptions, or outcomes change
- When updating `AGENTS.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`, or other core
  docs
- When proposing new migration increments

## Files and their roles

| File                      | Purpose                                    | Update frequency                               | Entry format                                          |
| ------------------------- | ------------------------------------------ | ---------------------------------------------- | ----------------------------------------------------- |
| `MIGRATION.md`            | Long-term plan, phases, exit criteria      | When phases complete or strategy shifts        | Section-based; mark completed phases                  |
| `MIGRATION-LOG.md`        | Historical record of completed increments  | Immediately after each increment               | Reverse chronological; present tense; concise         |
| `MIGRATION-INCREMENTS.md` | Living backlog of next immediate work      | When increments are completed or scope changes | Numbered sections; includes "why" and review targets  |
| `AGENTS.md`               | Agent behavior rules and workflows         | When guidance changes; skills kept in sync     | Sections; links to other docs; emphasis with 🔴       |
| `CONTRIBUTING.md`         | Shared expectations, testing, verification | When practices evolve                          | Sections; links to tools; emphasis with bullet points |
| `ARCHITECTURE.md`         | Design decisions and module boundaries     | When architecture changes                      | Sections; design rationale; decision tradeoffs        |

## When to update which file

**`MIGRATION.md`:**

- A phase's exit criteria have been met.
- Assumptions about the migration have changed.
- The scope of a phase needs adjustment.
- Decisions made during implementation affect long-term strategy.

**`MIGRATION-LOG.md`:**

- Immediately after an increment is completed.
- Entry format: `YYYY-MM-DD: <short title> - <what changed in present tense>`
- Example: `2026-05-02: Add environment-backed config values - Accept explicit required environment declarations...`
- No explicit coverage-outcome narration if the entry already states
  tests/behavior.

**`MIGRATION-INCREMENTS.md`:**

- Remove completed increments immediately.
- Renumber remaining increments if deletions create gaps.
- Add new increments when backlog is exhausted (see backlog exhaustion protocol
  in `AGENTS.md`).
- Each increment includes: title, "Why this increment exists", "Review target"
  bullets.
- Keep focused on immediate next work (not the entire remaining migration).

**`AGENTS.md`:**

- When agent behavior rules change (e.g., new verification requirements).
- When workflow guidance needs clarification.
- When skills are added or updated (keep in sync with actual skills).
- Links to relevant `CONTRIBUTING.md` sections.

**`CONTRIBUTING.md`:**

- When testing/verification practices evolve.
- When shared design expectations change.
- When new tools or commands are introduced.
- Keep in sync with available `./scripts/` wrappers.

**`ARCHITECTURE.md`:**

- When module boundaries are reorganized.
- When design decisions are finalized or revised.
- When the "Current State" vs. "Target State" progresses.
- Include rationale and tradeoffs for decisions.

## Update timing and batching

**Immediate, not batch:**

- After each increment completes, update both `MIGRATION-LOG.md` and
  `MIGRATION-INCREMENTS.md` in the same session.
- Do not defer documentation updates.

**Bundled with related changes:**

- If a single change affects multiple files (e.g., a feature change plus its
  testing guidance), update all affected docs in the same commit.
- After documentation changes, check for consistency across all docs.

## Link conventions

- **Internal links** use relative paths: `[CONTRIBUTING.md](./CONTRIBUTING.md)`
  or `[rule name](./CONTRIBUTING.md#section-anchor)`.
- **No markdown in section anchors** — Use lowercase with hyphens:
  `#test-code-guidelines`, not `#Test Code Guidelines`.
- **File paths in increments** use backticks for code blocks but plain paths
  when embedded in prose: `` `rust/src/lib.rs` `` vs. `MIGRATION-INCREMENTS.md`.

## Entry format reference

**`MIGRATION-LOG.md` entry:**

```
- YYYY-MM-DD: <short title> - <what changed, present tense>
```

**`MIGRATION-INCREMENTS.md` increment:**

```markdown
### Increment N: <Title>

Why this increment exists:

- <Context or rationale point 1>
- <Context or rationale point 2>

Review target:

- <What should be reviewed / expected outcome 1>
- <What should be reviewed / expected outcome 2>
```

**`AGENTS.md` entry:**

```markdown
- **Rule name** — Short description. Link to [relevant section](./CONTRIBUTING.md#anchor)
  if applicable. Use 🔴 emoji for critical/non-negotiable rules.
```

## Consistency check

After updating any core documentation file, verify:

1. **Links are valid** — `./.agent/scripts/lint-md.sh` confirms links resolve.

2. **No contradictions** — If a rule changed in `CONTRIBUTING.md`, update any
   mirrored guidance in `AGENTS.md` or vice versa.

3. **Increment numbering is sequential** — If removing increments from
   `MIGRATION-INCREMENTS.md`, renumber remaining ones.

4. **Skills are in sync** — If `AGENTS.md` or `CONTRIBUTING.md` change, review
   and update related skills in `.agent/skills/`.

5. **Phase progress is reflected** — If work completes a phase in
   `MIGRATION.md`, mark it as such and verify exit criteria language matches
   what was achieved.

## Skill maintenance

When updating any guideline or instruction file:

1. Identify skills that reference the changed section.
2. Review skill text and adjust references or explanations if needed.
3. Do not duplicate rule text; instead, update links or reference descriptions.
4. If a skill's guidance has become outdated or misleading, update it before
   claiming the primary documentation change is complete.

Skills currently in scope:

- `migration-workflow-rust` — References verification, increment scope, and
  commit message format. Update if those change.
- `gitenv-documentation-management` — This skill. Self-referential; update if
  documentation roles or conventions change.

## Common patterns

**Proactive documentation updates:**

- Do not wait for a human to ask. If scope changes during implementation,
  update `MIGRATION-INCREMENTS.md` immediately.
- If a decision shifts architecture assumptions, suggest updating
  `ARCHITECTURE.md`.

**Deferred work:**

- If follow-up work is discovered (e.g., significant coverage drop), document
  it in `MIGRATION-INCREMENTS.md` as a new item or as an inline TODO.
- Do not create separate tracked items outside the migration docs.

**Explanation vs. decision:**

- `MIGRATION.md` is for long-term strategy. Individual increments go in
  `MIGRATION-INCREMENTS.md`.
- `ARCHITECTURE.md` is for design rationale. Specific implementation decisions
  go in `AGENTS.md` or inline comments.
