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

### Increment 10a: Recover include planning coverage after include rollout

Why this increment exists:

- Increment 10 introduces include parsing/planning behavior and new branches,
  and overall line coverage drops from 97.92% to 94.48%.
- The current tests cover primary include semantics but leave several fallback
  and error paths under-covered.

Review target:

- Add focused tests for include planning edge branches, especially around
  non-`ReadConfiguration` include load errors and root-path-seeded cycle
  detection.
- Add direct unit assertions for include-path diagnostic determinism (sorted,
  deduplicated reporting).
- Restore coverage trend upward from the post-Increment-10 baseline while
  preserving existing include behavior.

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
- Make sure that each individual test has been migrated to the new module
  structure and add doc comments to test modules and functions where helpful for
  clarity.
- Take the time to thoroughly review test code against test code guidelines and
  fix any issues to ensure the tests are of sufficient quality and
  maintainability for the next increments to build on. Pay particular attention
  to test naming and assertion quality.

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
