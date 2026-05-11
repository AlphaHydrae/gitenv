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

### Increment 50: Extract info and apply orchestration modules

Why this increment exists:

- `lib.rs` still mixes public exports, composition-root wiring, and the
  `run_info`/`run_apply` command flows, which makes the file harder to review.
- The dependency-boundary refactor needs a stable home for command-level tests
  before command contexts replace the remaining ad hoc injections.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Move `run_info` and `run_apply` plus their associated tests into dedicated
  modules under a directory (for example `app/info.rs` and `app/apply.rs`).
- Keep `lib.rs` as the composition root and public export surface.
- Keep config path discovery and config loading in `lib.rs`; the extracted
  command modules should operate on already-loaded config/runtime state rather
  than owning bootstrap logic.
- Preserve current behavior and keep the existing command seams temporarily if
  that keeps the move reviewable.

### Increment 51: Introduce command-owned orchestration contexts

Why this increment exists:

- `SystemCalls` and the command-level `load_config` closures overlap with the
  new boundary direction and keep command orchestration on ad hoc injections.
- Once `info` and `apply` live in dedicated modules, they should depend on one
  focused context each instead of threading multiple closures through helper
  signatures.

Review target:

- Follow the end-state contract in
  [Dependency Boundary Refactor Target](./MIGRATION.md#dependency-boundary-refactor-target).
- Replace command-level closure bundles such as `SystemCalls` and injected
  config loaders with command-owned contexts that bundle resolved runtime data
  and wired stage entrypoints.
- Use resolved facts in those contexts (for example `LoadedConfig` and
  resolved `home_directory`) instead of deferred bootstrap closures whenever
  practical.
- Make the command modules call those wired planning/execution entrypoints
  instead of lower-level DI seams directly.
- Remove the temporary guarded-apply wiring test from `lib.rs` once boundary
  unit tests and command-module coverage prove it no longer covers anything
  unique.
- Preserve current command behavior and coverage while reducing orchestration
  duplication.

### Increment 52: Remove the intent injectable wrapper

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
- Remove the temporary public-entrypoint wrapper test from `lib.rs` once the
  migrated intent tests prove the same path through the remaining public API.
- Carry `home_directory` as stage context data instead of threading it through
  helper signatures as a separate argument.
- Preserve intent behavior and coverage while reducing exported surface area.

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
