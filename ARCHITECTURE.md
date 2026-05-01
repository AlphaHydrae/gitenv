# Architecture

## Goals

- Manage environment configuration files from a repository in a predictable,
  safe way.
- Make all functionality available both as a Rust library API and as a CLI.
- Support both symlink and copy workflows with clear status reporting.
- Keep configuration human-readable and declarative.
- Keep business logic testable and decoupled from terminal rendering.
- Provide thorough diagnostics with configurable log level.
- Build a maintainable implementation first; optimize performance only when
  needed.

## Current State

The current implementation is Ruby-based and uses a Ruby DSL config file
(`~/.gitenv.rb`).

## Target State

The next implementation will be written in Rust and use a declarative YAML
configuration format validated by schema.

## Design Decisions

### Language transition: Ruby to Rust

Rust is chosen for long-term maintainability, type safety, and easier
cross-platform distribution. The migration is behavior-oriented rather than a
line-by-line port.

### Platform scope

Initial support is Unix only. The implementation should isolate platform-specific
filesystem behavior behind clear boundaries so Windows support can be added
later without redesign.

### Configuration format

Ruby DSL is replaced by YAML. The format is declarative and source-oriented,
with optional shorthand for readability.

Canonical shape:

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
```

Optional shorthand:

```yaml
sources:
  - from: "."
    configs:
      - .zshrc
      - file: .tmux
        as: .tmux.conf
```

All shorthand is normalized to one canonical internal model before planning or
execution.

### Configuration compatibility policy

The Rust implementation does not execute Ruby config files and does not support
Ruby DSL compatibility mode.

### No hot reload

Configuration reload happens per command invocation only. Long-running
configuration watchers are out of scope.

### Initial configuration creation behavior

The Rust implementation does not need to replicate the previous interactive
first-run configuration creation flow. When configuration is missing or invalid,
the CLI must print clear setup instructions and example config paths instead.

### Correctness over performance

Correct behavior, understandable code, and diagnostics are prioritized over
micro-optimizations.

### API stability sequencing

Stabilize the canonical data model and programmatic API before extending feature
surface.

### Library and CLI parity

Features are implemented in the library layer first, then exposed by the CLI.
The CLI should act as a thin adapter over library capabilities instead of owning
exclusive behavior.

### Logging model

Logging must be thorough and configurable.

- The CLI exposes log level configuration via command options.
- The library exposes log level configuration via API.
- Domain modules emit structured events and diagnostics; renderer/transport
  choices are configured at boundaries.

### Output boundary: domain data vs CLI rendering

Business modules must not write directly to terminal output.

Business modules are responsible for:

- Parsing and validating configuration.
- Building deterministic execution plans.
- Executing filesystem operations and returning structured outcomes.

CLI modules are responsible for:

- Human-readable output formatting.
- Colorized status rendering.
- Command-level error display and exit behavior.

This boundary keeps business logic reusable and testable without terminal side
effects.

## Proposed Module Boundaries (Rust)

- `config`: parse + schema validate + normalize input.
- `plan`: convert normalized config into execution plan.
- `actions`: symlink/copy operations.
- `status`: structured status model.
- `logging`: log level filtering and structured diagnostic emission contracts.
- `cli`: argument parsing and output rendering.

## Testing Strategy

- Unit tests for parser, schema validation, normalization, and planners.
- Integration tests for filesystem behavior on Unix temporary directories.
- End-to-end tests for CLI workflows and output contracts.
- Coverage goal: 100% (or documented justified gap).

## Future Considerations

- Windows support via platform abstraction boundaries.
- Additional configuration schema versions with explicit migrations.
- Optional machine-readable output mode for automation.
