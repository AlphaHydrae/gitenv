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
  coverage decreases, explain why and document follow-up work in the living
  increments backlog and/or inline TODO comments.
- Avoid mixing architectural refactors and feature delivery in the same
  increment unless explicitly approved.

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

- [ ] `ARCHITECTURE.md` documents decisions and boundaries.
- [ ] `CONTRIBUTING.md` defines testing and verification expectations.
- [ ] `AGENTS.md` aligns agent behavior with repository standards.

### Phase 1: Parser, schema, and planning model

- Implement YAML parser and schema validation.
- Implement normalization from shorthand to canonical model.
- Implement execution planning (without filesystem side effects).
- Define and lock the canonical public library API for planning and execution.

Exit criteria:

- [ ] Parser unit tests cover valid/invalid examples.
- [ ] Normalization tests verify shorthand and canonical equivalence.
- [ ] Plan snapshots cover core combinations.

### Phase 2: Filesystem execution

- Implement symlink operations (Unix).
- Implement copy operations.
- Implement overwrite/backup/mkdir semantics.
- Implement status model for "info" and "apply" workflows.

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

## Test Strategy

- Use unit tests for parser, schema validation, normalization, and utilities.
- Use integration tests for filesystem behavior in temporary directories.
- Use end-to-end tests for CLI behavior and output contracts.
- Use golden/snapshot tests for normalized plans and user-facing diagnostics.
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

- [x] `./scripts/run-tests.sh`
- [x] `./scripts/run-lint.sh`
- [x] `./scripts/run-build.sh`
- [x] `./scripts/run-format.sh`
- [x] `./scripts/run-lint-md.sh`
- [x] `./scripts/run-coverage.sh`

Exact command mappings (including non-wrapper commands) are documented in
`CONTRIBUTING.md`. Wrapper output logs are written under `tmp/agent/`.
