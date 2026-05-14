# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.
It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned. The overall migration plan is located in
[`MIGRATION.md`](./MIGRATION.md).

## Increment Workflow Rules

### Coverage Baseline And Regression Rules

- For every increment, capture a coverage baseline **BEFORE** editing tracked
  files.
- If baseline capture is missed, recover it from a detached temporary worktree
  at `HEAD` under `tmp/agent/` instead of using `git stash` in the active
  working tree.
- Treat the baseline as the value captured from the branch state at increment
  start (or recovered from detached `HEAD` per policy), not from an earlier
  conversation snapshot.
- Do not infer or restate a baseline from memory; cite the exact command output
  captured for the current increment.
- If post-change coverage is lower than that baseline, keep the increment open
  until coverage is restored or the project owner explicitly approves the drop.
- Capture and report follow-up work for significant coverage decreases and other
  gaps in increment scope:
  1. if follow-up is a discrete task, create a new increment with clear scope
     and review target,
  2. if follow-up is open-ended, add inline TODO comments and consider adding a
     note in the next increment that references those TODOs.
- Do not add new uncovered code, or cause a coverage drop in code modified by
  the current increment, without explicit human approval.

### Completion And Evidence Gate

- Do not mark an increment complete while any Review target bullet remains
  partially implemented, deferred without agreement, or unverified.
- Do not remove an increment from this file or add its log entry until all of
  the following evidence exists in the same working pass:
  1. required wrapper checks executed (`tests`, `lint`, `build`, `format`,
     `coverage`, and `lint-md` when docs changed),
  2. command outputs show success exit codes,
  3. coverage baseline and post-change values are both reported.
- If scope changes mid-increment, rewrite the increment entry before claiming
  completion so the backlog reflects the real agreed scope.

### Backlog Hygiene

- Completed increments must be moved to [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
  and removed from this file so the backlog stays forward-looking.

## Documentation

Make sure to understand the project by reading appropriate sections of the
following documents before starting work on an increment:

- [Architecture & design decisions](./ARCHITECTURE.md)
- [Contribution guidelines](./CONTRIBUTING.md)
- [Agent instructions](./AGENTS.md)

## Design Decisions

Architectural and design decisions referenced by active increments:

- [Two-stage planning model](./ARCHITECTURE.md#two-stage-planning-model)
- [Library and CLI parity](./ARCHITECTURE.md#library-and-cli-parity)
- [Output boundary: domain data vs CLI rendering](./ARCHITECTURE.md#output-boundary-domain-data-vs-cli-rendering)

## Current Backlog

### Increment 62: Add `existing_directories_only` for recursive `select`

Why this increment exists:

- Recursive selection needs a way to keep planned actions visible without
  creating missing target directories. This slice adds a select-scoped policy
  for workflows that only want recursive entries whose target directories are
  already present.
- `existing_directories_only` is the chosen name for this policy. It means the
  planner keeps nested entries in the plan, marks entries whose target
  directory is absent as grayed out with their prospective source and target
  paths, and lets apply skip those actions instead of creating directories.

Review target:

- Add `existing_directories_only: bool` to `SelectConfig` and
  `IntentSelectAction`, defaulting to `false`.
- Keep recursive selection as the only mode affected by this policy; direct
  file actions and non-recursive selection keep their current mkdir behavior.
- In operation planning, mark recursive selector entries whose target
  directory does not already exist as unavailable-or-skipped entries that still
  carry their prospective source and target paths for rendering.
- In info rendering, show those entries grayed out with the same path pair the
  eventual operation would use.
- In apply execution, skip the concrete filesystem operation for those entries
  instead of treating the missing target directory as an error.
- Add focused tests for config parsing, intent propagation, info rendering, and
  apply skipping behavior.

### Increment 63: Make `select.exclude` use glob patterns

Why this increment exists:

- Recursive selection is most useful with path-aware filtering. Moving from
  exact-name exclusion to glob patterns enables practical recursive workflows
  without adding includes yet.

Review target:

- Replace exact-string exclusion checks with glob-pattern matching for
  `select.exclude`.
- Match recursive candidates using source-relative paths (for example
  `**/*.tmp`, `private/**`, `**/.DS_Store`).
- Keep deterministic behavior and define stable precedence between selector
  type filtering and glob excludes.
- Add unit tests for representative glob cases in both direct and recursive
  modes.
- Add one integration test validating glob excludes in recursive apply output.
- Preferred library: `globset` (widely used in Rust tooling, deterministic,
  supports `**` and efficient compiled pattern sets).
