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

### Increment 49: Introduce focused context traits for planner and apply paths

Why this increment exists:

- Dependency-heavy signatures and recursive helper plumbing in `intent.rs`
	(`too_many_arguments`) reduce readability and increase maintenance cost.
- Function-pointer injection scales poorly as more filesystem/config
	responsibilities are added.

Review target:

- Introduce small, responsibility-scoped traits (for example: env/config read,
	directory probing/listing, apply filesystem operations) and replace large
	argument lists with trait-backed contexts.
- Refactor `intent`, `operation`, and `actions` internals to consume those
	contexts without changing domain behavior.
- Add or update unit tests to validate deterministic behavior with trait-based
	test doubles.

### Increment 50: Reduce public DI seam surface and hide test-only wiring

Why this increment exists:

- The crate root currently re-exports several injectable variants that are
	primarily useful for internal tests, increasing API surface area and
	discoverability noise.
- Public API stability is not a concern for this migration stage.

Review target:

- Keep a minimal public surface around primary entrypoints
	(`derive_intent_plan`, `derive_operation_plan`, `apply_operation_plan`) and
	move test-only wiring helpers to `pub(crate)` or private scope.
- Update integration and unit tests to use supported seams (module-local tests,
	trait doubles, or higher-level public entrypoints) instead of exported
	internals.
- Ensure CLI/library behavior parity remains fully covered after API reduction.

### Increment 51: Extract environment/system boundary and rebalance adapters

Why this increment exists:

- `SystemCalls` in `lib.rs` currently mixes environment concerns into the
	composition root while related responsibilities are split across
	`fs_adapter.rs` and `path_resolution.rs`.
- The current boundary makes ownership of HOME/env/path logic unclear.
- Runtime adapters like `target_exists` and `create_symlink_on_filesystem`
	currently live in `actions.rs` but are wired from `lib.rs` (composition
	root), which blurs module responsibility boundaries.

Review target:

- Move environment/system lookup (`SystemCalls` replacement) into a dedicated
	module and inject it from `lib.rs`.
- Keep `fs_adapter` focused on filesystem operations and expand
	`path_resolution` ownership for shared path normalization where appropriate.
- Evaluate whether `target_exists` and `create_symlink_on_filesystem` in
	`actions.rs` should be moved to a dedicated system-calls or platform module,
	or whether they should stay internal to `actions.rs` with a simpler
	composition wrapper exposed.
- Add unit tests that lock module responsibilities and preserve current
	behavior for HOME, config path resolution, includes, and target/source
	resolution.

### Increment 52: Readability cleanup and naming pass for DI architecture

Why this increment exists:

- After boundary refactors, naming and documentation drift can keep the code
	difficult to review even when behavior is correct.

Review target:

- Rename DI types/functions for intent-revealing responsibilities and remove
	obsolete transitional naming.
- Add concise module/function docs that explain where wiring happens and where
	domain logic begins.
- Verify no coverage drop from refactor fallout and document any deferred
	cleanup as explicit TODOs with closure conditions.

<!-- Increment 47 (expand ~ in guards and includes) is complete. -->
