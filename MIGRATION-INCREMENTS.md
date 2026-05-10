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

### Increment 49: Extract shared dependency boundary traits

Why this increment exists:

- The current intent-stage trait objects are local to `intent.rs`, which makes
  the dependency boundary look stage-specific even though the same external
  capabilities will be reused by operation and apply.
- `lib.rs` still owns ad hoc wiring, so the final dependency boundary is not
  explicit yet.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Introduce a small internal boundary module for external dependencies and
  move the concrete adapters there.
- Define the final shared trait set for the migration boundary: environment
  reads, directory existence probes, config reads, directory listing, target
  existence probes, and symlink creation.
- Keep path normalization as ordinary utility code, not as a trait.

### Increment 50: Remove the intent injectable wrapper

Why this increment exists:

- `derive_intent_plan_with_injectables` is transitional once the shared trait
  boundary exists, and leaving it public keeps the API noisier than necessary.
- The intent stage should be driven by one public entrypoint plus a private
  internal implementation.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Replace the public injectable intent function with an internal function that
  takes the shared context directly.
- Keep `derive_intent_plan` as the sole public intent entrypoint and update
  tests to construct local trait-backed contexts instead of calling the removed
  wrapper.
- Carry `home_directory` as stage context data instead of threading it through
  helper signatures as a separate argument.
- Preserve intent behavior and coverage while reducing exported surface area.

### Increment 51: Rebalance operation boundary

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
- Move the operation-stage wiring out of `lib.rs` so the composition root only
  assembles real adapters and passes them to the operation planner.
- Keep `home_directory` resolved in composition root and stored in operation
  context data.
- Keep `path_resolution.rs` as a pure helper module and preserve current HOME
  and filesystem behavior.

### Increment 52: Rebalance apply boundary

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
- Keep `home_directory` resolved in composition root and stored in apply
  context data when needed by path-related helper flows.
- Preserve current filesystem behavior and conflict handling semantics.

### Increment 53: Readability cleanup and naming pass for DI architecture

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
