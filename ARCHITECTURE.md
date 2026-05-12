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

### Dynamic behavior without executable config

The Ruby DSL allowed runtime behavior in configuration code. The Rust port must
preserve those behavior outcomes while keeping YAML non-executable.

Core supported mechanisms in the YAML model:

- Environment-backed values and required environment declarations.
- Declarative conditional guards (for example, apply only when a target path
  exists).
- Deterministic include/composition support for splitting private/shared
  config.
- Runtime selection expansion (for example dotfile selectors with exclusions)
  during planning.

These are treated as first-class configuration semantics in the planner, not
as arbitrary scripting hooks.

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

During migration, shape the Rust library API for clarity and correctness rather
than compatibility. Backward compatibility becomes a concern only after the
migration is complete and the migration-planning documents are retired.

### Two-stage planning model

Planning is represented in two deterministic stages.

- Intent plan: config-shaped data after shorthand normalization and default
  resolution. This stage keeps user intent explicit and side-effect free.
- Operation plan: execution-shaped data with flat concrete copy/symlink
  operations, produced after resolving repository-relative sources and
  home-relative targets into explicit filesystem paths.

This split keeps planning testable and predictable while still making the final
execution model action-oriented.

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
- Building deterministic intent plans and operation plans.
- Executing filesystem operations and returning structured outcomes.

CLI modules are responsible for:

- Human-readable output formatting.
- Colorized status rendering.
- Command-level error display and exit behavior.

This boundary keeps business logic reusable and testable without terminal side
effects.

## Composition Root

The composition root in `lib.rs` manages all wiring of external dependencies
and stage entrypoints.

The production entry points are:

1. **`run(args)`** — Parses CLI arguments and runs the selected command.
2. **`run_cli(cli)`** — Runs a command from already-parsed CLI arguments.
3. **`run_info(runtime_config)`** — Runs the info workflow.
4. **`run_apply(runtime_config)`** — Runs the apply workflow.
5. **`derive_intent_plan(...)`** — Derive an intent plan from parsed
   configuration.
6. **`derive_operation_plan(...)`** — Derive an operation plan from an intent
   plan and file system state. This is where runtime path resolution and
   selection expansion happens.
7. **`inspect_operation_plan_status(operation_plan)`** — Compare an operation
   plan against filesystem state to produce a structured status report.
8. **`apply_operation_plan(...)`** — Apply an operation plan to the filesystem
   and produce a structured report of outcomes.

Each entry point handles composition concerns (runtime resolution and dependency
wiring where needed) and then delegates to the implementation modules.

### Stage-owned contexts:

Each planning/execution stage receives a dedicated context object containing
the runtime inputs and external capabilities needed by that stage. This keeps
stage behavior deterministic and testable while keeping composition concerns
centralized.

## Module Boundaries

- **`boundary`**: Defines dependency boundaries for external/system concerns and
  provides production and test implementations.
- **`app/`**: Owns top-level command orchestration flows.
- **`config`**: Parses, validates, and normalizes configuration input.
- **`intent`**: Builds deterministic intent plans from normalized config.
- **`operation`**: Expands intent plans into concrete filesystem operations.
- **`actions`**: Executes planned filesystem changes.
- **`status`**: Computes structured inspection/status results.
- **`logging`**: Handles diagnostic emission and log-level behavior.
- **`cli`**: Parses CLI arguments and renders user-facing output.
- **`path_resolution`**: Resolves and expands filesystem path semantics.
- **`fs_adapter`**: Encapsulates direct filesystem utility interactions.
- **`color`**: Centralizes color policy and terminal color behavior.
- **`errors`**: Defines shared error types used across modules.

## Testing Strategy

- Unit tests for parser, schema validation, normalization, and planners.
- Integration tests for filesystem behavior on Unix temporary directories.
- End-to-end tests for CLI workflows and output contracts.
- Coverage goal: 100% (or documented justified gap).

## Future Considerations

- Support creating links for entire relative directory trees, not just
  individual files.
- Additional configuration schema versions with explicit migrations.
- Windows support via platform abstraction boundaries.
- Optional machine-readable output mode for automation.
