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
- Aim for 100% test coverage (slightly under is acceptable if justified).

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
  coverage decreases significantly, explain why and document follow-up work in
  the living increments backlog and/or inline TODO comments. Treat a drop of
  about 0.25 percentage points or more as significant unless there is a
  stronger project-specific reason.
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
          dotfiles: true
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
          dotfiles: true
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
- [ ] No filesystem mutation executor exists yet for symlink or copy
      operations, so apply semantics (including overwrite/backup/mkdir
      behavior) are still pending.

Exit criteria:

- [ ] Integration tests validate behavior against fixture directories.
- [ ] Error messages are stable and documented.

### Phase 3: CLI and UX parity

- Implement command interface (`info` default and `apply`).
- Implement colorized output and status rendering.
- Implement clear setup instructions for missing/invalid config (non-
  interactive).
- Ensure CLI commands map directly to equivalent library API operations.
- Implement configurable logging with log level controls in CLI and library.

Current status:

- [x] The CLI currently supports the default no-arg inspection path and
      prints setup guidance when config loading fails.
- [ ] Explicit `info`/`apply` command parsing, colorized rendering parity,
      and configurable logging controls are still pending.

Exit criteria:

- [ ] End-to-end CLI tests cover primary workflows.
- [ ] README examples run as documented.

### Phase 4: Hardening and release readiness

- Add coverage reporting and enforce threshold.
- Add wrapper scripts for test/lint/build/format/documentation lint.
- Add CI matrix and release workflow.

Exit criteria:

- [ ] Coverage target reached (goal: 100%).
- [ ] CI green for required checks.
- [ ] Release checklist documented.

## Pending Refactorings

This section tracks refactorings that are planned but intentionally deferred to
keep migration increments focused and reviewable. Each refactoring should be
implemented in a future increment.

- [ ] Normalize error names (ask for guidance).
- [ ] Do not copy files when the target file already matches (hash).
- [ ] Improve action tests by reading the whole temporary test directory state.

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

- Goal: 100% line and branch coverage.
- If coverage is below 100%, document precise uncovered paths and rationale.

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
