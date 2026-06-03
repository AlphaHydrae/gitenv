# Migration Increments (Living Plan)

This document contains only the immediate, actively managed migration backlog.
It is intentionally not written in stone. As implementation progresses and we
learn more, this backlog should be split, merged, reordered, rewritten, and
pruned. The overall migration plan is located in
[`MIGRATION.md`](./MIGRATION.md).

## Increment Workflow Rules

### Coverage Regression Rules

- Run coverage checks for increments that change code.
- If post-change coverage is lower than 100%, keep the increment open until
  coverage is restored or the project owner explicitly approves the drop.
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
  3. coverage output confirms the coverage target is still met.
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

The increments below come from the 2026-06-03 maintainability review. The
intended end state is documented in the
[Maintainability and Test Organization Refactor Target](./MIGRATION.md#maintainability-and-test-organization-refactor-target)
section of the migration plan. They are ordered by priority and are independent
unless noted; each should land on its own. The test-organization increments
(71-73) only move or share existing test code, so coverage must remain at 100%.

### Increment 71: Extract inline unit-test modules into sibling test files

Why this increment exists:

- Several source files are dominated by large inline `#[cfg(test)] mod tests`
  blocks (for example `rust/src/intent.rs` is about 3,150 lines, roughly 2,600
  of which are tests). This makes the production logic hard to find and the
  files hard to navigate.
- These tests exercise private items, so they cannot move to the `tests/`
  integration crate without widening visibility. Moving them to a sibling file
  that stays a child module keeps full private access with no production change.

Review target:

- For each affected module, replace the inline `#[cfg(test)] mod tests { ... }`
  block with `#[cfg(test)] mod tests;` and move the body to
  `rust/src/<module>/tests.rs`, beginning the file with `use super::*;`.
- Apply this to the largest offenders first: `intent`, `operation`, `actions`,
  `config`, `status`, `cli`, and `lib`.
- Use the existing `rust/src/operation.rs` plus `rust/src/operation/`
  file-and-directory layout as the reference pattern.
- Make no production-code or visibility changes; this increment only relocates
  test code.
- Confirm coverage stays at 100% function/line/region and all wrapper checks
  pass, since no test was added or removed.

### Increment 72: Split oversized test files into themed submodules

Why this increment exists:

- After increment 71 the test bodies live in their own files, but the largest
  ones are still thousands of lines in a single file and remain hard to read.

Review target:

- Break each large `tests.rs` into themed submodules under a `tests/`
  directory (for example `rust/src/intent/tests/{defaults,includes,guards,
  selectors}.rs`) declared from a small `tests.rs` module root.
- Group tests by behavior area so each file is a few hundred focused lines.
- Make no production-code changes and keep coverage at 100%.

### Increment 73: Centralize duplicated test fixtures and helpers

Why this increment exists:

- Test fixtures are duplicated across modules: the `available_operation_plan`
  helper is copied in five places, `gitenv_command_for_home` in three, and
  plan/config model builders and tempdir setup are re-implemented per file.
- Two of the four integration-test files do not use `tests/support.rs`, which is
  why command-builder and config helpers were re-copied there.

Review target:

- Introduce a crate-internal `#[cfg(test)] mod test_support` (for example
  `rust/src/test_support.rs`) that exposes the shared plan/config builders and
  tempdir helpers used by unit tests.
- Route the integration tests (`integration.rs`, `actions_apply.rs`,
  `cli_output.rs`, `readme_examples.rs`) through `tests/support.rs` and remove
  their duplicated command-builder and config helpers.
- Keep behavior identical and coverage at 100%; this is a deduplication pass.

### Increment 74: Decide policy on the 100% region-coverage threshold

Why this increment exists:

- The 100% region-coverage gate is the root cause of the test volume and the
  white-box, internals-coupled style of the tests. Region coverage forces tests
  for defensive and near-unreachable branches and tightly couples tests to
  internal structure.
- Whether to keep this bar is a project-owner policy decision, not a unilateral
  change, because the coverage rules forbid lowering coverage without approval.

Review target:

- Present the trade-offs of the three options: keep 100% function/line/region as
  is; keep 100% line/function but relax the region dimension to an agreed floor;
  or keep 100% with targeted exclusions on genuinely defensive code.
- Record the owner's decision in the migration plan and, if the bar changes,
  update the `coverage` wrapper thresholds and document the rationale.
- Identify any tests that exist only to satisfy region accounting so they can be
  reconsidered once the policy is settled.

### Increment 75: Adopt insta snapshot tests for plan and config expectations

Why this increment exists:

- Plan and config tests hand-build large expected structures and compare them
  with `assert_eq!` (for example `rust/src/core_plan_snapshots_test.rs` and the
  large expected-config table in `rust/tests/readme_examples.rs`). These are
  verbose to write and to keep in sync.

Review target:

- Add `insta` as a dev-dependency and convert the plan/config golden tests to
  `insta::assert_debug_snapshot!` (or equivalent), checking in the generated
  snapshots.
- Keep the existing assertions until the snapshots are reviewed and accepted, so
  behavior coverage is not reduced.
- Confirm coverage remains at 100% after the conversion.

### Increment 76: Parameterize repetitive matrix tests with rstest

Why this increment exists:

- Several test groups repeat near-identical functions across a matrix of inputs
  (for example the symlink/copy times skip/overwrite/backup conflict-policy
  cases in `rust/tests/actions_apply.rs`).

Review target:

- Add `rstest` as a dev-dependency and collapse the most repetitive matrices
  into parameterized cases.
- Keep one case per meaningful combination so coverage stays at 100%.

### Increment 77: Single-pass CLI argument parsing

Why this increment exists:

- `cli.rs` parses the argument vector twice (once via `get_matches_from` on a
  clone and once via `parse_from`) solely to recover the `ValueSource` of the
  `--repo` argument. This clones the arguments and creates two parse paths that
  must stay in agreement.

Review target:

- Parse once with `get_matches_from`, build the `Cli` via `from_arg_matches`,
  and read the `repo` value source from the same `matches`.
- Preserve the existing repository precedence behavior (flag, then env, then
  config) and its debug logging.
- Keep coverage at 100% and behavior unchanged.

### Increment 78: Make the filesystem boundary trait surface consistent

Why this increment exists:

- `DirectoryProbe::is_directory` takes `&str` while every other boundary trait
  takes `&Path`, forcing lossy `to_string_lossy` conversions at call sites.
- Operation planning probes target-directory existence by calling `is_dir`
  directly on the real filesystem, bypassing the boundary that the rest of the
  stage uses and making that branch depend on real filesystem state in tests.

Review target:

- Change `DirectoryProbe::is_directory` to accept `&Path` and update callers and
  test doubles accordingly.
- Route the target-directory existence probe in `operation.rs` through a
  boundary trait instead of calling the filesystem directly.
- Preserve current behavior and keep coverage at 100%.

### Increment 79: Adopt thiserror for the error type

Why this increment exists:

- `errors.rs` hand-writes a large `Display` implementation for the multi-variant
  `ProgramError` enum and an empty `Error` impl. This is boilerplate that a
  derive macro can express more concisely, and the current design flattens I/O
  errors into strings with no error-source chain.

Review target:

- Add `thiserror` and replace the manual `Display`/`Error` impls with
  `#[error("...")]` attributes per variant.
- Reconsider the `PartialEq`/`Eq` derive on `ProgramError`, which currently
  exists for test assertions and prevents embedding non-`Eq` error sources; if
  it is dropped, migrate affected tests (snapshot or message-based assertions).
- Keep error messages stable (they are documented and asserted) and coverage at
  100%.

### Increment 80: Remove cross-module duplication and dead indirection

Why this increment exists:

- Small duplications and dead indirection accumulated across modules: the ANSI
  escape constants are defined in both `cli.rs` and `logging.rs`, the
  `IntentPlanner`/`OperationPlanner` type aliases are duplicated across
  `app/apply.rs` and `app/info.rs`, and `encode_color_mode` is an identity
  function.

Review target:

- Define the ANSI escape constants once (for example in `color.rs`) and reuse
  them.
- Define the planner type aliases once in a shared location and reuse them in
  the `app` modules.
- Inline or remove the `encode_color_mode` identity helper.
- These are trivial, local cleanups; keep behavior unchanged and coverage at
  100%.
