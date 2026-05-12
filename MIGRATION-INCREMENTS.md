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

### Increment 56: Add `--config PATH` CLI flag

Why this increment exists:

- The Ruby binary accepts `-c`/`--config PATH` to override the config file path
  directly on the command line. The Rust port only supports the `GITENV_CONFIG`
  environment variable for this purpose, which is a usability regression for
  one-off invocations and shell scripts.

Review target:

- A `--config` (`-c`) flag is added to the `Cli` struct in `cli.rs`.
- When provided, it takes precedence over `GITENV_CONFIG` and the default
  XDG/HOME paths (precedence order: `--config` > `GITENV_CONFIG` > XDG >
  default).
- `default_config_path` (or its caller in `lib.rs`) is updated to receive and
  apply the override.
- A unit test covers the new precedence branch.
- An integration test verifies that `gitenv --config <path> info` resolves the
  named file.

### Increment 57: Auto-exclude `.DS_Store` on macOS during selector expansion

Why this increment exists:

- Ruby's `Context` silently adds `.DS_Store` to every selector's ignore list
  when running on macOS. The Rust port requires users to list it explicitly in
  each `select.exclude` array. This is a silent behavioral difference that
  affects every macOS user migrating an existing config.

Review target:

- When expanding a `select` action on macOS (detected via
  `cfg(target_os = "macos")`), `.DS_Store` is excluded by default unless the
  user has already listed it in `exclude`.
- The exclusion is applied in the `should_include_selection_entry` helper in
  `operation.rs` (or via a platform constant injected alongside
  `IntentSelectAction`).
- A unit test guarded with `#[cfg(target_os = "macos")]` asserts that
  `.DS_Store` entries are omitted from the expanded result.
- A cross-platform note is added to the `select` documentation in
  `rust/README.md`.

### Increment 58: Add "select all files" mode to cover dotfiles and non-dotfiles

Why this increment exists:

- Ruby's `AllFiles` matcher returns every file in the source directory
  regardless of whether the name starts with a dot. The Rust `select` only
  supports `dotfiles: true` (names starting with `.`) and `dotfiles: false`
  (names not starting with `.`). A user migrating a `symlink all_files` call
  must currently write two separate `select` items, which is unintuitive and
  not documented.

Review target:

- The `select` item gains an `all_files: true` option that includes every entry
  regardless of dot-prefix, subject to `exclude` filtering as normal.
- The canonical internal model (`IntentSelectAction`) carries the new field.
- `should_include_selection_entry` in `operation.rs` is updated accordingly.
- Existing `dotfiles: true` and `dotfiles: false` behavior is unchanged (no
  regression).
- Parser tests and at least one operation-expansion test cover the new mode.
- `rust/README.md` documents `all_files: true` with a short example.

### Increment 59: Pre-flight source existence check before apply

Why this increment exists:

- Ruby's `check_files!` iterates all configured actions before any filesystem
  mutation and aborts with a grouped diagnostic if any source file is missing
  or unreadable. The Rust port surfaces missing source files as individual
  operation errors at apply time — one per operation with no pre-flight summary
  gate. A user with a misconfigured `file:` entry only discovers it mid-apply.

Review target:

- A new function (for example `preflight_check_operation_plan` in `status.rs`
  or a new `preflight.rs` module) iterates all `OperationAction` entries and
  collects source paths that do not exist or are not readable.
- When problems are found, a new `ProgramError` variant (e.g.
  `MissingSourceFiles { paths: Vec<PathBuf> }`) is returned with all problem
  paths grouped.
- `run_apply` in `app/apply.rs` calls the pre-flight check before executing any
  operations.
- `run_info` is not required to run the pre-flight check (Ruby's `check_files!`
  was apply-side only); a comment in `run_info` notes the intentional omission.
- A unit test asserts that a plan containing a missing source file is rejected
  with the new error before any apply work begins.
- An integration test verifies the grouped error message against the compiled
  binary.
