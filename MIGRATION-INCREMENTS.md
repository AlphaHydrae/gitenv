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

### Increment 69: Continue coverage recovery after apply-path branch tests

Why this increment exists:

- Follow-up coverage work has raised total line coverage to 99.93%, but a few
  live-path and assertion regions in `rust/src/lib.rs`, `rust/src/actions.rs`,
  `rust/src/status.rs`, and `rust/src/app/apply.rs` still keep the broader
  recovery work open.

Review target:

- Improve total line coverage above the current 99.93% baseline without
  changing delivered runtime behavior.
- Prefer unit tests over integration tests while closing the remaining
  uncovered regions that the fresh JSON report still shows in the following
  hotspots:
  - `rust/src/actions.rs`:
    - `overwrite_copy_when_target_contents_differ` still leaves uncovered
      assertion regions around the expected `ApplyOperationOutcome::Applied`
      value for the overwrite-copy success path.
    - `report_copy_failures` still leaves uncovered assertion regions around
      the `ProgramError::FileCopyFailed` propagation check when copy creation
      cannot run.
    - `cannot_apply_symlink_when_directory_creator_fails` still leaves
      uncovered error-propagation regions around the `TargetDirectoryCreationFailed`
      path when parent-directory creation fails.
    - These misses are in test macro expansions and assertion regions, not new
      production branches.
  - `rust/src/lib.rs`:
    - `resolve_default_config_path_without_overrides` still leaves uncovered
      fallback-path regions for the home-directory default when no config
      overrides are present.
    - `cannot_run_with_a_whitespace_only_env_repository_override` still leaves
      uncovered validation regions around the whitespace-only repository
      override rejection.
  - `rust/src/status.rs`:
    - `report_missing_when_copy_target_is_absent` still leaves uncovered
      regions for the `CopyInspectionState::Missing` branch.
    - `report_not_a_file_when_copy_target_path_is_a_directory` still leaves
      uncovered regions for the `CopyInspectionState::NotAFile` branch.
    - The unix-gated symlink inspection test still leaves an uncovered
      metadata-success assertion region under `#[cfg(unix)]`.
  - `rust/src/app/apply.rs`:
    - `cannot_run_apply_when_apply_execution_fails` still leaves an uncovered
      orchestration region where planning succeeds but apply execution returns
      `OperationPlanBlocked`.
- Keep parsing concerns in `rust/src/cli.rs` and composition-root wiring in
  `rust/src/lib.rs`.
- Run required wrappers (`tests`, `lint`, `build`, `format`, `coverage`, and
  `lint-md` when docs change) and report fresh timestamp evidence.
