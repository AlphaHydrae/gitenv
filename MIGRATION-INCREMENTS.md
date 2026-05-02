# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.

It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned.

Completed increments must be moved to [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
and removed from this file so it stays forward-looking.

Capture and report follow-up work to address significant coverage decreases and
other gaps in the increment scope. If the follow-up is a discrete task, create
a new increment with a clear scope and review target. If the follow-up is more
open-ended, add inline TODO comments in the relevant code and consider adding a
note in the next increment that explicitly references the TODOs to ensure they
are not forgotten. Treat a drop of about 0.25 percentage points or more as
significant unless there is a stronger project-specific reason.

## Design Decisions

Architectural and design decisions referenced by active increments:

- [Two-stage planning model](./ARCHITECTURE.md#two-stage-planning-model)
- [Library and CLI parity](./ARCHITECTURE.md#library-and-cli-parity)
- [Output boundary: domain data vs CLI rendering](./ARCHITECTURE.md#output-boundary-domain-data-vs-cli-rendering)

## Current Backlog

### Increment 9: Add declarative filesystem guards

Why this increment exists:

- Existing Ruby configs gate actions with `File.directory?` checks.
- Declarative `when` guards should be introduced before write-side execution
  behavior expands.

Review target:

- Add schema/model support for declarative guard conditions.
- Resolve guard outcomes deterministically in planning without executing config
  code.
- Add planner tests covering both matched and unmatched guard cases.

### Increment 10: Add deterministic config includes

Why this increment exists:

- Existing Ruby configs compose additional private config through `include`.
- Composition semantics should be locked before broader planner and CLI work.

Review target:

- Add include support for YAML config composition with deterministic merge
  behavior.
- Detect include cycles and return stable, actionable diagnostics.
- Add parser/planning tests for include ordering, cycle rejection, and
  duplicate-handling behavior.

### Increment 11: Support per-config-item option overrides

Why this increment exists:

- Current planning applies global defaults uniformly to all config items.
- Users need item-level overrides for mode and destination behavior without
  duplicating sources.

Review target:

- File/select config items can override relevant default execution options.
- Planning resolves item-level overrides deterministically.
- Item-level explicit `overwrite: false` with `backup_on_overwrite: true` is
  rejected as an invalid combination, while inherited backup defaults remain
  valid when overwrite resolves to false.
- Shared resolved-option fields are factored into one reusable plan type to
  reduce duplication across planned action variants.

### Increment 12: Split planning/config code for readability

Why this increment exists:

- `rust/src/lib.rs` currently combines library API surface, config parsing,
  and execution-plan derivation in one large file.
- The file has no explanatory comments for non-trivial contracts and planning
  logic, which increases onboarding and review cost.
- Splitting and documenting this layer before more feature increments keeps the
  migration maintainable while preserving behavior.

Review target:

- Split parsing/config model code and planning derivation code into focused
  modules that match architecture boundaries.
- Keep `src/lib.rs` as a thin public API/re-export surface.
- Add concise doc comments on non-obvious public types/functions and targeted
  inline comments only for non-trivial planning branches.
- Preserve library/CLI behavior and keep coverage at least stable.

### Increment 13: Inspect one narrow symlink status case

Why this increment exists:

- Introduce the first real filesystem behavior with tightly constrained scope.
- Start with read-only inspection before write-side apply behavior.

Review target:

- A single symlink status workflow works against temporary directories.
- Status results are returned as structured data, not terminal output.

### Increment 14: Wire the default CLI inspection flow

Why this increment exists:

- Deliver the first thin end-to-end vertical slice through config, planning,
  inspection, and rendering.
- Confirm the CLI is acting as an adapter over library behavior.

Review target:

- The default CLI path loads config, inspects status, and renders output.
- CLI tests cover observable output and exit behavior.
