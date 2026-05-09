# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.
It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned. The overall migration plan is located in
[`MIGRATION.md`](./MIGRATION.md).

For every increment, capture a coverage baseline **BEFORE** editing tracked
files. If baseline capture is missed, recover it from a detached temporary
worktree at `HEAD` under `tmp/agent/` instead of using `git stash` in the active
working tree.

Completed increments must be moved to [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
and removed from this file so it stays forward-looking.

Capture and report follow-up work to address significant coverage decreases and
other gaps in the increment scope. If the follow-up is a discrete task, create
a new increment with a clear scope and review target. If the follow-up is more
open-ended, add inline TODO comments in the relevant code and consider adding a
note in the next increment that explicitly references the TODOs to ensure they
are not forgotten. Do not add new uncovered code, or cause a coverage drop in
code modified by the current increment, without explicit human approval.

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

### Increment 28: Update README and finalize Phase 3 exit criteria

Why this increment exists:

- The README still documents the Ruby CLI; Phase 3 requires README examples
  that match the Rust CLI.
- Phase 3 exit criteria ("end-to-end CLI tests cover primary workflows" and
  "README examples run as documented") must be directly verifiable.

Review target:

- Update README to document the Rust CLI commands with accurate examples.
- Add or expand end-to-end CLI tests so both Phase 3 exit criteria are met.
- Mark Phase 3 exit criteria as complete in MIGRATION.md.

### Increment 29: Normalize error names

Why this increment exists:

- "Normalize error names" is listed as a pending refactoring in MIGRATION.md.
- Consistent naming improves readability and makes error-handling code easier
  to follow.
- Requires human guidance on naming conventions before implementation.

Review target:

- Review all error variant names across `ProgramError` and any domain error
  types; agree on a consistent naming convention with the human.
- Rename variants to match the agreed convention.
- Preserve all behavior and test coverage.

### Increment 30: Reject empty sources and empty source configs

Why this increment exists:

- A configuration with `sources: []` or a source entry with `configs: []` is
  currently tolerated, but it is not meaningful user intent.
- Rejecting these shapes will make configuration errors clearer and reduce the
  need to reason about no-op configuration structures.

Review target:

- Decide whether the validation belongs in parsing, normalization, or intent
  derivation.
- Reject configs with no sources.
- Reject any source entry that declares no configs.
- Add targeted tests for both invalid shapes and the resulting error messages.

### Increment 31: Skip copy when target content already matches

Why this increment exists:

- "Do not copy files when the target file already matches (hash)" is listed as
  a pending refactoring in MIGRATION.md.
- Unnecessary copies waste I/O and can reset file metadata without reason.

Review target:

- Before overwriting in the copy path, compare source and target content via
  streamed hash (consistent with the existing inspection approach in `status.rs`).
- Skip the copy and report `SkippedExistingTarget` when hashes match.
- Add unit and integration tests covering the skip-on-match and overwrite-on-mismatch
  branches.

### Increment 32: Improve action tests to assert full directory state

Why this increment exists:

- "Improve action tests by reading the whole temporary test directory state"
  is listed as a pending refactoring in MIGRATION.md.
- Current integration tests in `actions_apply.rs` assert individual file
  outcomes; asserting the full directory state catches unintended side effects.

Review target:

- Extend selected integration tests in `actions_apply.rs` to read and assert
  the complete state of the temporary test directory after each operation.
- Ensure no regressions; no new behavior changes.
