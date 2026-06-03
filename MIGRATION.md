# Migration Plan: Ruby gitenv to Rust gitenv

This document defines the migration plan from the current Ruby implementation to
an upcoming Rust implementation, including configuration migration, rollout
strategy, and quality gates.

## Scope And Non-Goals

### In scope

- Replace the Ruby runtime implementation with a Rust CLI and library.
- Replace Ruby DSL configuration (`~/.gitenv.rb`) with declarative YAML.
- Keep core user behavior: inspect status and apply symlink/copy operations.
- Ensure every supported feature is available through both the library API and
  the CLI.
- Keep Unix support first, with code structure that can be extended to Windows
  later.
- Add thorough logging with configurable log levels via both CLI and library.

### Out of scope

- Backward compatibility for Ruby config execution.
- Hot reload of configuration files.
- Performance-first optimization work.
- Migrating the previous interactive initial config creation flow.

## Key Decisions (From Design Discussion)

- Target platform for initial release: Unix only.
- No Ruby configuration support in Rust.
- No hot reload.
- No interactive initial config creation migration; print clear setup
  instructions instead.
- Prioritize maintainability and correctness over performance.
- Stabilize the canonical data model and programmatic API first.
- Use a two-stage planning model: intent plan first, then operation plan.
- Require feature parity between library and CLI surfaces.
- Require configurable logging from both CLI and library.
- Maintain 100% function, line, and region coverage in repository checks.

## Implementation Cadence And Review Workflow

- Implement migration work in step-by-step, reviewable increments.
- Preferred increment size is usually 10-20 lines plus associated tests when
  practical.
- It is acceptable to land intermediate incomplete slices toward a feature, but
  each slice must still be coherent and reviewable.
- Each increment should compile and pass tests before moving to the next one.
- If temporary code is needed to keep the project compiling and tests passing,
  include it with a clear comment that it is temporary and planned for removal
  in a later increment.
- Use TODO comments freely for intentionally deferred functionality.
- During implementation and review, explain Rust-specific language features and
  ecosystem idioms that may not be obvious to contributors coming from Ruby,
  Java, JavaScript, and TypeScript.
- Preserve public behavior and API in each increment unless a behavior change
  is explicitly documented and approved for that increment.
- Before each increment, provide a short "why this increment exists" note.
- Prefer additive changes first and cleanup/removal in a following increment,
  unless cleanup is trivial and local.
- For each deferred concern, keep one clear TODO comment that includes a
  removal/closure condition.
- Add increment-level regression tests whenever bugs or edge cases are found.
- For every increment, report test coverage before and after the change. If
  coverage decreases, restore it in the same increment before considering the
  work complete unless the project owner explicitly approves a temporary drop;
  if a drop is approved, explain why and document follow-up work in the living
  increments backlog and/or inline TODO comments.
- Avoid mixing architectural refactors and feature delivery in the same
  increment unless explicitly approved.
- As increments are completed, keep phase exit-criteria checkboxes in this file
  up to date and mark criteria complete only when there is direct evidence in
  code, tests, and migration-log entries.

## Living Plan

The living migration increments backlog lives in
[MIGRATION-INCREMENTS.md](./MIGRATION-INCREMENTS.md) so it can be updated
frequently without destabilizing this long-term migration plan.

## Increment Log

Completed increments are tracked in [MIGRATION-LOG.md](./MIGRATION-LOG.md).

## Dependency Boundary Refactor Target

This section defines the intended end state for the dependency-injection and
composition-root refactoring tracked by increments 49-53 in
[MIGRATION-INCREMENTS.md](./MIGRATION-INCREMENTS.md).

### End-state goals

- Keep `lib.rs` as the composition root that resolves runtime state and wires
  real adapters.
- Move command orchestration out of `lib.rs` into focused modules under a
  directory (for example `app/info.rs` and `app/apply.rs`) so `lib.rs` stays
  focused on exports and top-level wiring.
- Keep config path discovery, config loading, and other bootstrap/runtime
  resolution in the composition root rather than inside command modules.
- Replace callback-style injectable seams with trait-backed, stage-owned
  contexts.
- Keep the public library surface centered on primary entrypoints rather than
  test-only wrapper functions.
- Preserve current behavior for intent planning, operation expansion, apply
  execution, path handling, and diagnostics.

### Shared boundary traits

The shared boundary module should expose only external/system capabilities that
are reused across stages:

- Environment variable reads.
- Directory existence probes.
- Config file reads.
- Directory listing for selector expansion.
- Target existence probes.
- Symlink creation.

Path normalization remains regular deterministic code in `path_resolution.rs`;
it is not part of the boundary trait surface.

### Stage-owned context shape

Each stage owns a focused context (for example: intent, operation, apply)
containing only the dependencies and runtime state needed by that stage.

Command orchestration should follow the same pattern: `info` and `apply`
modules should each receive a command-owned context that bundles resolved
runtime data and the already-wired stage entrypoints they need, rather than
reaching down to lower-level injectable seams directly.

Those command-owned contexts should prefer resolved runtime facts over deferred
bootstrap work. For example, they should receive `LoadedConfig` and resolved
`home_directory` data rather than injected config-loader closures.

`home_directory` is resolved once in the composition root and carried as data in
the stage context (not as a trait method), so helper signatures stay small and
the context fully represents stage runtime inputs.

### Function/API direction

- Internal stage functions accept context references instead of multiple
  callback arguments.
- Command orchestration modules accept command-owned contexts that expose the
  wired planning/execution entrypoints they need; they should not call
  lower-level test-oriented DI seams directly.
- The composition root resolves config path overrides/defaults, loads the
  config file, resolves `HOME`, and constructs the command context before
  handing control to `info` or `apply` orchestration modules.
- Transitional injectable wrappers are removed once tests are migrated to
  stage-local trait-based doubles.
- Public entrypoints stay stable around the primary stage APIs unless an
  explicitly approved increment states otherwise.

## Maintainability and Test Organization Refactor Target

This section captures a post-implementation maintainability review performed
during Phase 4 hardening (2026-06-03). It defines the intended end state for the
test-organization and code-quality cleanup tracked by increments 71+ in
[MIGRATION-INCREMENTS.md](./MIGRATION-INCREMENTS.md).

### Assessment summary

The Rust implementation is in good shape and broadly idiomatic. At review time
it was `clippy`-clean across all targets, `rustfmt`-enforced, passing all tests
(371), and built on a clean two-stage planner (`config` -> `intent` ->
`operation` -> `actions`/`status`) layered over a trait-based dependency
boundary. Production code is panic-free; all `unwrap`/`expect` calls live in
tests.

The dominant maintainability issue is test organization, not production logic:
the test-to-production line ratio is roughly 4:1, and unit tests live in very
large inline `#[cfg(test)] mod tests` blocks. For example, `src/intent.rs` is
about 3,150 lines, of which roughly 2,600 are one inline test module.

### Test organization problem and root cause

The size of the source files is a symptom. The driver is the 100% function,
line, and **region** coverage gate enforced by the `coverage` wrapper. Hitting
full region coverage requires exercising every branch of private helpers, which
forces white-box tests that need access to module internals. That access is the
reason the tests are inline (a child module can see its parent's private items;
the separate `tests/` crate cannot). The coverage policy therefore drives both
the test volume and the inline placement.

### Test organization end state

- Move each inline `#[cfg(test)] mod tests` body into a sibling file declared as
  `#[cfg(test)] mod tests;` and stored at `src/<module>/tests.rs`. The test
  module remains a child of the production module in the same crate, so it keeps
  full access to private items with no visibility widening. This is the same
  file-plus-directory module layout already used by `src/operation.rs` and
  `src/operation/directory_listing.rs`. Inline tests are not moved into the
  `tests/` integration crate, because that crate only sees the public API and
  would force internals to become `pub` purely for testing.
- Split oversized test files into themed submodules (for example
  `src/intent/tests/{defaults,includes,guards,selectors}.rs`) so each test file
  stays readable on its own.
- Centralize duplicated test fixtures and model builders into a crate-internal
  `#[cfg(test)] mod test_support`, and route the integration tests through the
  existing `tests/support.rs`, removing the repeated tempdir, command-builder,
  and plan-builder boilerplate.
- Revisit the 100% region-coverage threshold as an explicit, owner-approved
  policy decision. Options range from keeping it as-is, to relaxing the region
  dimension while keeping 100% line/function, to keeping 100% with targeted
  exclusions on genuinely defensive code. This is the largest available lever on
  test volume and must not be changed unilaterally under the coverage rules.
- Migrate hand-written golden expectations toward `insta` snapshot tests and
  collapse repetitive matrix tests with `rstest` parameterization to reduce the
  volume of hand-maintained expected data.

### Code-quality targets

These are smaller, localized improvements identified by the review:

- Replace the double argument parse in `cli.rs` (currently two full `clap`
  parses to recover one `ValueSource`) with a single-pass
  `from_arg_matches` approach.
- Make the filesystem boundary trait surface consistent: `DirectoryProbe`
  should take `&Path` like the other boundary traits instead of `&str`, and the
  direct `is_dir` probe in operation planning should go through the boundary
  rather than touching the real filesystem directly.
- Adopt `thiserror` for `ProgramError` to remove the hand-written `Display`
  boilerplate, and reconsider the `Eq` derive that currently forces stringly
  typed I/O errors with no error source chaining.
- Remove cross-module duplication (the ANSI escape constants defined in both
  `cli.rs` and `logging.rs`, the `IntentPlanner`/`OperationPlanner` type aliases
  duplicated across `app/apply.rs` and `app/info.rs`) and dead indirection (the
  identity `encode_color_mode` helper).

## Configuration Migration

## New Format: Declarative YAML

The Rust implementation will use YAML with schema validation.

Canonical example:

```yaml
version: 1
repository: ~/projects/env

defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true

sources:
  - from: "."
    configs:
      - file: .zshrc
      - file: .tmux
        as: .tmux.conf
      - select:
          type: dot
          exclude:
            - .DS_Store

  - from: a
    configs:
      - file: config
```

Optional shorthand (supported for readability):

```yaml
version: 1
repository: ~/projects/env

sources:
  - from: "."
    configs:
      - .zshrc
      - file: .tmux
        as: .tmux.conf
      - select:
          type: dot
```

The parser will normalize shorthand into one canonical internal model.

## Required Dynamic-Behavior Parity (Without Code Execution)

Existing Ruby configurations often use Ruby language features for dynamic
behavior. The Rust migration must preserve those outcomes through declarative
YAML fields and deterministic planning, not config code execution.

Required parity capabilities:

- Environment-provided source roots:
  Replace Ruby expressions like `from ENV[...]` with explicit placeholders or
  bindings that resolve from environment variables at load time.
- Required environment variables with clear diagnostics:
  Replace `raise`-style checks with schema-level `required_env` declarations
  and stable error messages when values are missing.
- Conditional actions based on filesystem existence:
  Replace Ruby `if File.directory?(...)` branches with declarative `when`
  conditions (for example: destination exists, source exists).
- Reusable includes/composition:
  Replace Ruby `include ...private...` with declarative include/merge support
  that is explicit, cycle-safe, and deterministic.
- Runtime-expanded selections:
  Preserve selection behavior (for example dotfile selection with exclusions)
  as declarative selectors expanded during planning.

These capabilities are part of core configuration semantics and must be modeled
before broad filesystem execution and CLI parity work continues.

Representative legacy Ruby example:

```ruby
# Shared dotfiles
symlink dot_files.except('.local-only', '.shell-profile'), overwrite: true, backup: false
copy('.local-only').once

# Tool-specific config from subdirectories
from 'editor' do
  symlink('settings.json').to('.config/editor')
  symlink('keybindings.json').to('.config/editor')
end

# Optional integration enabled only when the destination app directory exists
app_config_dir = File.expand_path(File.join('~', 'Library', 'Application Support', 'ExampleApp'))
if File.directory? app_config_dir
  from 'app' do
    symlink('data.json', overwrite: true).to(app_config_dir)
  end
end

# Private config loaded from an environment-provided directory
private_dir = ENV['PRIVATE_ENV_DIR']
raise 'Environment variable $PRIVATE_ENV_DIR is required' unless private_dir

from private_dir do
  symlink dot_files.except('.private-data', '.gitenv.extra.rb')
  copy('.private-data').once

  from '.ssh' do
    symlink('config').to('.ssh')
  end
end

include File.join(private_dir, '.gitenv.extra.rb')
```

This example demonstrates the behavior patterns the YAML model must preserve:
selection with exclusions, per-item copy/symlink behavior, filesystem-gated
actions, required environment input, and deterministic composition.

## Validation

- YAML files are validated against a schema.
- Schema errors must include clear paths and messages.
- Unknown keys are rejected by default to prevent silent misconfiguration.

## Migration Phases

### Phase 0: Documentation and design lock

- Finalize architecture and contribution rules for the new implementation.
- Finalize YAML structure and canonical internal model.
- Define configuration schema versioning strategy.

Exit criteria:

- [x] `ARCHITECTURE.md` documents decisions and boundaries.
- [x] `CONTRIBUTING.md` defines testing and verification expectations.
- [x] `AGENTS.md` aligns agent behavior with repository standards.

### Phase 1: Parser, schema, and planning model

- Implement YAML parser and schema validation.
- Implement normalization from shorthand to canonical model.
- Implement declarative runtime context resolution (env bindings, required env,
  conditional guards, and includes) with deterministic expansion.
- Implement intent planning with defaults resolved (without filesystem side
  effects).
- Implement operation planning that derives concrete actions after selection
  expansion.
- Define and refine the canonical public library API for planning and
  execution during migration.

Exit criteria:

- [x] Parser unit tests cover valid/invalid examples.
- [x] Normalization tests verify shorthand and canonical equivalence.
- [x] Dynamic-behavior parity tests cover env resolution, missing required env
      diagnostics, include behavior, and filesystem-gated conditions.
- [x] Intent and operation plan snapshots cover core combinations.

Current status:

- [x] Parser coverage, shorthand/canonical normalization, required
      environment resolution, filesystem-gated guards, and deterministic
      include behavior are implemented.
- [x] The intent-stage output is represented by `IntentPlan`, and the
      operation stage resolves concrete source/target operations.
- [x] Intent and operation snapshot coverage is implemented for representative
      shorthand/canonical parity, include ordering, guard evaluation, selector
      expansion, and item-level overrides.

### Phase 2: Filesystem execution

- Implement symlink operations (Unix).
- Implement copy operations.
- Implement overwrite/backup/mkdir semantics.
- Implement status model for "info" and "apply" workflows.

Current status:

- [x] A read-only symlink inspection slice exists and is wired into the
      default CLI inspection flow.
- [x] Filesystem mutation executors exist for symlink and copy operations,
      including `mkdir` and conflict-policy behavior (`skip`, overwrite, and
      backup-on-overwrite).

Exit criteria:

- [x] Integration tests validate behavior against fixture directories.
- [x] Error messages are stable and documented.

### Phase 3: CLI and UX parity

- Implement command interface (`info` default and `apply`).
- Implement colorized output and status rendering.
- Implement clear setup instructions for missing/invalid config (non-
  interactive).
- Ensure CLI commands map directly to equivalent library API operations.
- Implement configurable logging with log level controls in CLI and library.

Current status:

- [x] The CLI supports the default no-arg inspection path and explicit
      `info`/`apply` commands, and prints setup guidance when config loading
      fails.
- [x] Primary inspection/apply status states are colorized for terminal output
      and remain plain in non-terminal contexts.
- [x] Configurable logging controls are available in CLI (`--log-level`) and
      library (`init_logging`).
- [x] Temporary Rust-port usage examples live in `rust/README.md`.
- [x] Home-based CLI path rendering uses `~` display parity for paths under
      HOME.

Exit criteria:

- [x] End-to-end CLI tests cover primary workflows.
- [ ] Rust-port README examples run as documented.

### Phase 4: Hardening and release readiness

- [x] Add coverage reporting and enforce 100% function, line, and region
  thresholds in repository checks.
- Add CI matrix and release workflow.

Exit criteria:

- [x] Coverage target reached and maintained at 100%.
- [ ] CI green for required checks.
- [ ] Release checklist documented.

## Pending Refactorings

This section tracks refactorings that are planned but intentionally deferred to
keep migration increments focused and reviewable. Each refactoring should be
implemented in a future increment.

- [x] Normalize error names.
- [x] Do not copy files when the target file already matches (hash).
- [x] Improve action tests by reading the whole temporary test directory state.

The following refactorings come from the 2026-06-03 maintainability review and
are detailed in the
[Maintainability and Test Organization Refactor Target](#maintainability-and-test-organization-refactor-target)
section above (tracked by increments 71+):

- [ ] Reorganize inline unit tests into sibling test files and themed
  submodules.
- [ ] Centralize duplicated test fixtures into shared test-support modules.
- [ ] Revisit the 100% region-coverage threshold as an explicit policy
  decision.
- [ ] Adopt snapshot/parameterized test libraries (`insta`, `rstest`) to reduce
  hand-written test volume.
- [ ] Address code-quality targets: single-pass CLI parse, boundary trait
  consistency, `thiserror` adoption, and cross-module deduplication.

## Future Work

### Include path composition from env-backed directories

The Ruby DSL supports building an include path by joining an environment
variable with a relative file name:

```ruby
private_dir = ENV['PRIVATE_ENV_DIR']
include File.join(private_dir, '.gitenv.extra.rb')
```

The Rust `includes` field resolves an env-backed entry as the full path stored
in the variable; it does not support composing the path by appending a static
suffix to an env-provided directory:

```yaml
# Supported: env variable holds the full include path
includes:
  - env: GITENV_EXTRA_CONFIG
# Not supported: env variable provides only the directory; file name is fixed
```

Users who need this pattern should set an additional environment variable to the
full file path. Supporting composition would require a new include form and is
not planned for the current migration scope.

### Recursive selector traversal limits

- Add optional configuration to cap recursive selector traversal depth.
- Keep the default behavior unbounded when depth is not specified.
- Validate that depth settings remain deterministic and composable with selector
  type filtering and glob pattern filters.

## Test Strategy

- Use unit tests for parser, schema validation, normalization, and utilities.
- Use integration tests for filesystem behavior in temporary directories.
- Use end-to-end tests for CLI behavior and output contracts.
- Use golden/snapshot tests for intent plans, operation plans, and user-facing
  diagnostics.
- Add parity tests to ensure CLI workflows and library workflows produce
  equivalent outcomes.
- Add logging tests for level filtering and destination behavior.

Coverage target:

- Maintain 100% function, line, and region coverage in repository checks.
- If a change causes a drop, restore coverage in the same increment before the
  work is considered complete unless the project owner explicitly approves a
  temporary exception.

## Rollout Strategy

- Release Rust implementation as a new major version.
- Provide migration documentation and examples from Ruby DSL to YAML.
- Keep migration tooling optional; if added, it must be explicit and
  deterministic.

## Risks And Mitigations

- Risk: User confusion when Ruby config is no longer supported.
  Mitigation: clear migration guide, clear startup error messages, examples.

- Risk: Ambiguous configuration semantics.
  Mitigation: schema-first design and canonical model normalization.

- Risk: Platform differences for symlink behavior.
  Mitigation: Unix-first scope and explicit platform abstraction boundaries.

## Automation Helpers

Wrapper scripts are set up for the Rust project structure:

- [x] `./.agent/scripts/tests.sh`
- [x] `./.agent/scripts/lint.sh`
- [x] `./.agent/scripts/build.sh`
- [x] `./.agent/scripts/format.sh`
- [x] `./.agent/scripts/lint-md.sh`
- [x] `./.agent/scripts/coverage.sh`

Exact command mappings (including non-wrapper commands) are documented in
`CONTRIBUTING.md`. Wrapper output logs are written under `tmp/agent/`.
