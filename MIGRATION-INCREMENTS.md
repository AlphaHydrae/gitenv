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

### Increment 67: Directory symlink support

Why this increment exists:

- Real configurations symlink directories, not only files. The Rust source
  availability check in `fs_adapter.rs` currently rejects directory sources,
  causing those entries to be reported as unavailable during apply.

Review target:

- Extend `ensure_source_path_readable` in `fs_adapter.rs` to accept directory
  sources when the action is a symlink operation.
- Extend the apply executor in `actions.rs` to call `create_symlink` for
  directory sources using the same conflict-policy semantics as file symlinks.
- Add unit tests covering a directory source that is symlinked, skipped on
  conflict, and overwritten with backup.
- Add an integration test that places a directory in the repository and verifies
  the symlink appears at the expected target location.

### Increment 68: Repository binding from env variable

Why this increment exists:

- The Ruby CLI supports overriding the repository root at runtime via
  `GITENV_REPO` env and `--repo PATH` flag. The Rust implementation requires
  the repository path to be declared in YAML. Users migrating from the Ruby
  tool lose this runtime override workflow.

Review target:

- Support a `GITENV_REPO` environment variable (and a `--repo` CLI flag) that
  overrides the `repository` field declared in the config at runtime.
- Keep the YAML `repository` field functional as the declared default; the
  env/flag override is additive and takes precedence when set.
- Propagate the resolved repository root to intent and operation planning
  without changing the canonical data model.
- Add unit tests verifying that the env variable takes precedence over the
  config field and that the config value is used when the variable is absent.
- Document the override in the Rust README.
