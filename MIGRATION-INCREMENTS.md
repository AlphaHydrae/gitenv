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

### Increment 58: Replace `dotfiles` selector boolean with `type` scope enum

Why this increment exists:

- Ruby's `AllFiles` matcher returns every file in the source directory
  regardless of whether the name starts with a dot. The Rust `select` only
  supports a `dotfiles: true|false` split, which makes "all files" behavior
  awkward and does not provide an explicit selector scope model.

Review target:

- The `select` item replaces `dotfiles` with `type: dot|non-dot|all`.
- The canonical internal models (`SelectConfig` and `IntentSelectAction`) carry
  the new selector-scope enum instead of a boolean.
- `should_include_selection_entry` in `operation.rs` is updated to branch on
  `type` (`dot`, `non-dot`, `all`) and still applies `exclude` filtering.
- Parser tests and operation-expansion tests cover all three selector modes.
- `rust/README.md` documents `type: dot|non-dot|all` with concise examples.

### Increment 59: Pre-flight source existence check before apply

Why this increment exists:

- Ruby's `check_files!` iterates all configured actions before any filesystem
  mutation and aborts with a grouped diagnostic if any source file is missing
  or unreadable. The Rust port surfaces missing source files as individual
  operation errors at apply time — one per operation with no pre-flight summary
  gate. A user with a misconfigured `file:` entry only discovers it mid-apply.

Review target:

- A new function (for example `preflight_check_operation_plan` in `status.rs`
  or a new `preflight.rs` module) iterates all `OperationAction` entries and
  collects source paths that do not exist or are not readable.
- When problems are found, a new `ProgramError` variant (e.g.
  `MissingSourceFiles { paths: Vec<PathBuf> }`) is returned with all problem
  paths grouped.
- `run_apply` in `app/apply.rs` calls the pre-flight check before executing any
  operations.
- `run_info` is not required to run the pre-flight check (Ruby's `check_files!`
  was apply-side only); a comment in `run_info` notes the intentional omission.
- A unit test asserts that a plan containing a missing source file is rejected
  with the new error before any apply work begins.
- An integration test verifies the grouped error message against the compiled
  binary.
