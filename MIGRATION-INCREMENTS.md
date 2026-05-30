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

### Add README examples for undocumented configuration and CLI features

**Why this increment exists**

Increments 1 and 2 identified and validated existing 10 README examples. Seven
categories of fully implemented configuration and CLI features remain
undocumented in the README. This increment adds YAML example blocks and
corresponding test scenario expectations for each feature gap to close the
documentation-to-implementation parity gap.

**What is completed**

1. **Environment-backed source roots** (`from: $VAR` shorthand and canonical
   object form `from: { env: VAR, optional: true }`) — Implemented in
   `rust/src/config.rs` lines 91–112.

2. **Source-level guards** (`when: to_exists` and `when: { directory_exists: path }`)
   — Implemented in `rust/src/config.rs` lines 149–172.

3. **Recursive selector and existing_directories_only flag** — Implemented
   in `rust/src/config.rs` lines 302–304, validated at line 388.

4. **Select item-level execution overrides** (`to`, `mkdir`, `overwrite`,
   `backup_on_overwrite` on individual select items) — Supported in
   `rust/src/config.rs` lines 310–318.

5. **File item-level mkdir override** (`mkdir: true` on individual file items)
   — Supported in `rust/src/config.rs` line 288.

6. **Optional includes and environment-backed include forms** (both `Path { path, optional: true }`
   and `Environment { env, optional: true }`) — Implemented in
   `rust/src/config.rs` lines 187–209.

7. **CLI feature examples** (explicit `info` subcommand invocation, `--config` flag usage,
   `--log-level` flag with examples, `--color` flag with examples) — Implemented in
   `rust/src/cli.rs` lines 70–103.

**Review target**

- [ ] Add YAML example blocks to `rust/README.md` for each of the 7 feature gaps
- [ ] Add corresponding test scenario IDs and expectations to
      `rust/tests/readme_examples.rs` expected_configs function
- [ ] Verify all scenario tests pass (`readme_examples_execute_without_error_in_temporary_directories`)
- [ ] Coverage maintained at 100% line/function/region
- [ ] All wrapper checks pass (build, lint, tests, format, lint-md, coverage)

**Coverage impact:** New test expectations added to existing test infrastructure;
coverage should remain at 100% function/line/region.
