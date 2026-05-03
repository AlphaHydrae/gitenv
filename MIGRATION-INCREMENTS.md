# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.
It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned. The overall migration plan is located in
[`MIGRATION.md`](./MIGRATION.md).

Completed increments must be moved to [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
and removed from this file so it stays forward-looking.

Capture and report follow-up work to address significant coverage decreases and
other gaps in the increment scope. If the follow-up is a discrete task, create
a new increment with a clear scope and review target. If the follow-up is more
open-ended, add inline TODO comments in the relevant code and consider adding a
note in the next increment that explicitly references the TODOs to ensure they
are not forgotten. Treat a drop of about 0.25 percentage points or more as
significant unless there is a stronger project-specific reason.

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

### Increment 20: Implement symlink conflict semantics

Why this increment exists:

- Symlink apply cannot be considered complete without deterministic conflict
  handling.
- `skip`/`overwrite`/backup behavior and `mkdir` are core semantics from the
  planning model.

Review target:

- Extend symlink apply behavior to honor conflict policies and `mkdir`.
- Add deterministic errors and integration coverage for each branch.

### Increment 21: Introduce copy apply execution with option parity

Why this increment exists:

- Operation planning already emits copy actions.
- Execution parity requires a concrete copy path with the same option semantics
  as symlink actions.

Review target:

- Add concrete copy execution that respects `mkdir` and conflict policy
  behavior.
- Add Unix fixture/integration tests that mirror symlink executor
  expectations.

### Increment 22: Expand status modeling for info and apply workflows

Why this increment exists:

- Status modeling currently centers on symlink inspection only.
- Broader status modeling is needed before full info/apply CLI parity.

Review target:

- Generalize structured status/output models beyond symlink inspection.
- Ensure info/apply surfaces can report typed per-operation outcomes for
  symlink and copy actions.

### Increment 23: Add explicit CLI command parsing for info and apply

Why this increment exists:

- The CLI is currently default-path inspection only.
- Command-level parity requires explicit `info` and `apply` entry points.

Review target:

- Replace implicit default-only flow with a thin CLI adapter that maps `info`
  and `apply` commands directly to library APIs.
- Keep deterministic error rendering.

### Increment 24: Add logging controls and finalize CLI UX parity

Why this increment exists:

- Configurable logging is a documented architecture requirement.
- Phase 3 parity also requires output/UX completion with testable behavior.

Review target:

- Introduce a `logging` module and CLI/API log-level controls.
- Add colorized renderer coverage for primary status states.
- Update README examples/tests so Phase 3 exit criteria are directly
  verifiable.
