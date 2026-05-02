---
name: migration-workflow-rust
description: Plan, implement, verify, and document Rust migration increments. Use when working on incremental feature delivery, verifying code changes, running tests and coverage, reporting results, and proposing next work.
---

# Migration Workflow: Rust Incremental Delivery

This skill guides the process of planning, implementing, verifying, and
documenting Rust migration increments for the gitenv project.

## When to use this skill

- Starting work on a new Rust migration increment
- Verifying increment work before claiming completion
- Reporting coverage before/after an increment
- Proposing new increments when the backlog is exhausted

## Increment planning

Before starting an increment:

1. **Understand the "why"** — Review the increment description in
   `MIGRATION-INCREMENTS.md` and the related entries in `MIGRATION-LOG.md` to
   understand design decisions and context.

2. **Check scope** — Increments should be 10–20 lines plus associated tests when
   practical. If the increment feels larger, ask whether it should be split.

3. **Identify review targets** — Each increment includes a "Review target"
   section describing what should be reviewed. Keep this in mind throughout
   implementation.

4. **Plan for temporary code if needed** — If temporary code is required to keep
   the project compiling and tests passing, mark it clearly with a TODO comment
   including the removal condition.

## Implementation

1. **Apply guidelines during changes, not after** — Keep the relevant sections
   from `CONTRIBUTING.md` open while editing. Review the exact rule before each
   change.

2. **For Rust code** — Ensure:
   - No `expect`, `unwrap`, `panic!`, or `unreachable!` without explicit
     approval (propagate `Result`/`Option` with `?` instead).
   - Public types/functions with non-obvious roles get doc comments.
   - Non-trivial logic blocks get targeted inline comments (not self-explanatory
     code).
   - Coherent responsibility boundaries are preserved (see
     readability/documentation gate in `AGENTS.md`).

3. **For tests** — Ensure:
   - Test names follow sentence-style imperative snake_case per test code
     guidelines in `CONTRIBUTING.md`.
   - Each test makes complete assertions for the behavior under test.
   - All test naming rules are checked before finalizing.

## Verification

Run these checks from the repository root (in order):

1. **Build** — `./.agent/scripts/build.sh`
   - Confirms compilation without errors.
   - Output captured in `tmp/agent/build_output.log`.

2. **Tests** — `./.agent/scripts/tests.sh`
   - Confirms all tests pass.
   - Output captured in `tmp/agent/test_output.log`.

3. **Lint** — `./.agent/scripts/lint.sh`
   - Confirms code style and clippy warnings.
   - Output captured in `tmp/agent/lint_output.log`.

4. **Coverage** — `./.agent/scripts/coverage.sh`
   - Reports total line coverage before and after.
   - Output captured in `tmp/agent/coverage_output.log`.
   - If coverage decreases by ~0.25 percentage points or more, document
     follow-up work in `MIGRATION-INCREMENTS.md` and/or inline TODO comments.

5. **Documentation** — If any Markdown files changed:
   - `./.agent/scripts/lint-md.sh`
   - Output captured in `tmp/agent/markdown_lint_output.log`.

**Show exit codes and summary output for every check.** Do not claim tests pass
without showing actual output.

## Commit message format

When an increment is complete, provide a suggested commit message:

1. **Imperative title** (≤72 characters) stating what changed, not why.
2. **Body** (optional) describing the change in bullet points if multiple
   meaningful changes exist. Focus on product/code behavior, not maintenance
   steps.
3. **Omit** routine test additions (already evident from diff) and migration log
   updates (unless documentation is the primary deliverable).

See commit message expectations in `AGENTS.md` for full guidance.

## After completion

1. **Update migration log** — Add an entry in `MIGRATION-LOG.md` describing what
   changed (present tense, no explicit coverage narration).

2. **Update backlog** — Remove or rewrite the increment in
   `MIGRATION-INCREMENTS.md`.

3. **Check for next steps** — If the backlog is exhausted, re-read
   `MIGRATION.md` and `MIGRATION-LOG.md`, then proactively suggest the next set
   of increments. See backlog exhaustion protocol in `AGENTS.md`.

## Common issues

- **File is too large** — If you notice a file growing large or mixing concerns,
  proactively surface it as maintainability debt. Propose either a scoped
  increment or a backlog item per proactive maintainability guidance in
  `AGENTS.md`.

- **Coverage drop without justification** — Explain why in a comment or
  increment note. Treat ~0.25pp or more as significant.

- **Test name unclear** — Re-read spec description style in `CONTRIBUTING.md`
  and rename. Test names are behavioral documentation.

- **Temporary code not marked** — Add a TODO comment with a removal condition
  and ensure it's mentioned in the increment description.
