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

### Increment 53: Rebalance operation boundary

Why this increment exists:

- `operation.rs` still uses callback-style seams that do not match the final
  dependency boundary.
- `lib.rs` still owns operation-stage wiring instead of delegating through a
  dedicated boundary module.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Refactor operation planning to consume the shared boundary traits or a
  stage-owned context instead of bare callbacks.
- Remove the remaining `SystemCalls` bootstrap shim from `lib.rs` and replace
  its env/config-path responsibilities with the shared boundary/context path so
  command/bootstrap wiring uses one dependency model end-to-end.
- Move the operation-stage wiring out of `lib.rs` so the composition root only
  assembles real adapters and passes them to the operation planner.
- Keep `home_directory` resolved in composition root and stored in operation
  context data.
- Keep `path_resolution.rs` as a pure helper module and preserve current HOME
  and filesystem behavior.

### Increment 54: Rebalance apply boundary

Why this increment exists:

- `actions.rs` still uses callback-style seams that do not match the final
  dependency boundary.
- The apply path should be isolated from the operation refactor so each stage
  remains reviewable on its own.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Refactor apply execution to consume the shared boundary traits or a
  stage-owned context instead of bare callbacks.
- Move the apply-stage wiring out of `lib.rs` so the composition root only
  assembles real adapters and passes them to the apply executor.
- After apply boundary refactoring, keep `app/apply.rs` tests focused on
  command orchestration via fakes and avoid re-testing lower-layer apply/action
  behavior already covered in stage tests.
- Keep `home_directory` resolved in composition root and stored in apply
  context data when needed by path-related helper flows.
- Preserve current filesystem behavior and conflict handling semantics.

### Increment 55: Readability cleanup and naming pass for DI architecture

Why this increment exists:

- After the boundary refactors, transitional names and documentation can still
  make the code harder to review even if the behavior is correct.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Rename boundary and context types for intent-revealing responsibility names.
- Add concise module/function docs that explain where wiring happens and where
  domain logic begins.
- Verify no coverage drop from refactor fallout and document any deferred
  cleanup as explicit TODOs with closure conditions.
