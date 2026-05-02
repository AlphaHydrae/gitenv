# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.

It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned.

Completed increments must be moved to [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
and removed from this file so it stays forward-looking.

Capture and report follow-up work to address coverage decreases and other gaps
in the increment scope. If the follow-up is a discrete task, create a new
increment with a clear scope and review target. If the follow-up is more
open-ended, add inline TODO comments in the relevant code and consider adding a
note in the next increment that explicitly references the TODOs to ensure they
are not forgotten.

## Current Backlog

### Increment 5: Reject unknown keys with clear errors

Why this increment exists:

- Enforce the schema-first boundary and avoid silent misconfiguration.
- Make config errors actionable before more behavior depends on them.

Review target:

- Unknown top-level and nested keys are rejected.
- Error messages are readable and stable enough to test.

### Increment 6: Normalize shorthand into the canonical model

Why this increment exists:

- Introduce the first meaningful domain transformation without filesystem side
  effects.
- Prove that shorthand and canonical YAML representations converge to one
  internal model.

Review target:

- Normalization code exists and is tested.
- Equivalent shorthand and canonical configs yield equivalent normalized data.

### Increment 7: Add an execution plan model without side effects

Why this increment exists:

- Separate deciding what should happen from doing it.
- Keep future filesystem behavior narrow and testable.

Review target:

- The library can derive a deterministic plan from normalized config.
- Plan output is structured and suitable for snapshots or golden tests.

### Increment 8: Inspect one narrow symlink status case

Why this increment exists:

- Introduce the first real filesystem behavior with tightly constrained scope.
- Start with read-only inspection before write-side apply behavior.

Review target:

- A single symlink status workflow works against temporary directories.
- Status results are returned as structured data, not terminal output.

### Increment 9: Wire the default CLI inspection flow

Why this increment exists:

- Deliver the first thin end-to-end vertical slice through config, planning,
  inspection, and rendering.
- Confirm the CLI is acting as an adapter over library behavior.

Review target:

- The default CLI path loads config, inspects status, and renders output.
- CLI tests cover observable output and exit behavior.
