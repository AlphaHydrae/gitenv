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

- Find a way to cover the 20 region-only gaps by architectural simplification,
  defensive code removal, or closure merging.

- Close the 3 remaining missed regions (as of 2026-05-30):

  - **`operation.rs` — 2 gaps** at lines 420–421 inside
    `list_directory_children`, on the `?` operator error branches for
    `read_dir_entry` and `read_entry_type` within the `for entry in read_dir`
    loop. These require a directory iterator that succeeds initially then fails
    mid-iteration, which requires filesystem failure injection not currently
    supported by the test structure. The `^0` column markers appear in the
    coverage text report at those lines.

  - **`status.rs` — 1 gap** at line 146 inside
    `inspect_symlink_operation_status_with_injectables`, on the `?` operator
    error branch of `let current_target = read_symlink_target(&operation.target)?;`.
    This is a monomorphization gap: the test `cannot_inspect_symlink_status_when_readlink_fails`
    already covers the error path for the test-double instantiation, but the
    real-filesystem instantiation (`read_symlink_target_from_filesystem`) has
    its error branch uncovered. To close it, add a test that exercises the
    injectable variant directly with a failing `read_symlink_target` closure
    when the target kind is `TargetKind::Symlink`.
