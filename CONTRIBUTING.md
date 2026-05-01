# Contributing

## Start Here

Before making changes, review:

- [README.md](./README.md) for project overview and usage.
- [ARCHITECTURE.md](./ARCHITECTURE.md) for design goals and decisions.
- [MIGRATION.md](./MIGRATION.md) for the Ruby-to-Rust transition plan.
- [MIGRATION-LOG.md](./MIGRATION-LOG.md) for completed migration increments.
- [MIGRATION-INCREMENTS.md](./MIGRATION-INCREMENTS.md) for the living next-step
  migration backlog.

Agent contributors must also follow [AGENTS.md](./AGENTS.md).

## Shared Expectations

- Align changes with the design decisions in [ARCHITECTURE.md](./ARCHITECTURE.md).
  If the required change appears to conflict with those decisions, raise the
  conflict explicitly and resolve it before proceeding.
- If you introduce a new cross-cutting concern, shared pattern, or reusable
  utility that should be followed elsewhere, document it so the convention does
  not remain implicit.
- Fix behavior that clearly diverges from expected language or framework
  semantics when it is safe and in scope. If the fix would be broader than the
  current change, call it out explicitly.
- Keep library and CLI capabilities aligned; avoid adding behavior that exists
  only in one surface unless explicitly documented and approved.
- Favor maintainability and correctness over premature optimization.
- Prefer existing language and standard-library utilities over custom
  reimplementations when they satisfy the requirement.

## Documentation Expectations

- Document the purpose of important classes and types when their role is not
  obvious from naming or surrounding code.
- Document non-trivial functions whose contract is not immediately clear from
  the function name.
- Add inline comments for non-trivial logic blocks only when they materially
  reduce the effort needed to understand the code.
- Do not add comments for trivial or self-explanatory code.
- Suggest or add missing documentation when a new file, class, or function
  would benefit from it.
- Suggest or make improvements when existing documentation is outdated or
  unclear.
- Keep documentation consistent across `README.md`, `ARCHITECTURE.md`,
  `CONTRIBUTING.md`, `MIGRATION.md`, `MIGRATION_LOG.md`,
  `MIGRATION_INCREMENTS.md`, and `AGENTS.md`.

## Boundaries

Business modules must not print directly to terminal output.

- Business modules should return structured data/results.
- CLI modules own output rendering (stdout/stderr formatting and colors).
- Logging must be configurable by log level in both library and CLI entry
  points.
- For missing or invalid configuration, print clear setup instructions instead
  of relying on interactive setup.

## Dependency Injection Principles

- Inject dependencies rather than constructing them inside business logic.
- In unit tests, use fakes/mocks for injectable dependencies.
- Keep runtime composition in a clear composition root at the CLI boundary.

## Testing Guidelines

- Write tests in behavior-focused language.
- Prefer clear, externally observable outcomes over implementation detail checks.
- Cover edge cases and failure modes explicitly.
- Coverage target is 100%; if lower, document uncovered paths and rationale.
- Include parity tests for equivalent library and CLI workflows.
- Include logging tests for level filtering and expected diagnostic visibility.

## Test Code Guidelines

### General style

- Describe the expected behavior or outcome rather than the action under test.
- Use `describe` blocks sparingly and only when they improve the structure of
  the tests. Do not group tests by method name.
- Keep test names human-readable and avoid technical details that do not help
  explain the expected behavior. Put necessary technical context in inline
  comments instead.
- For test function names, use sentence-style `snake_case` with an imperative
  verb at the start (for example:
  `show_the_default_message_on_stdout`). Avoid third-person singular forms such
  as `shows_...`.

### Spec description style

- Do not include method names, return values, or implementation details in test
  descriptions. Focus on describing the observable behavior and outcomes
  instead.
- Describe observable outcomes in plain language.
- Avoid return-value and state-machine framing where a behavioral sentence is
  clearer.
- Prefer concise verb-led behavior phrases for most examples.
- Keep names focused on externally observable behavior and avoid implementation
  terms such as `returns`, `checks`, `calls`, or explicit method names.
- Use `cannot` for constraints and failure-mode examples.
- Use plain imperative verb phrases for action-oriented examples.

### Output side-effect tests

- In business-layer tests, assert structured return values and emitted events
  instead of asserting terminal output side effects.
- In CLI-layer tests, assert output behavior at the CLI boundary only
  (`stdout`, `stderr`, log level gating, and rendering decisions).
- Keep parseable `stdout` contracts stable. When adding output modes, include
  tests that verify `stderr` diagnostics do not corrupt parseable `stdout`.

## Verification Checklist

Apply checks that match your change scope.

1. Ensure behavior is correct to the best of your ability.
2. Run relevant tests.
3. Run linting checks.
4. Run build checks.
5. Run documentation lint checks when Markdown files changed.
6. Run formatting checks (and formatting write mode only when intended).
7. Run coverage for code changes. If coverage decreases, capture required
   follow-up work (living increment backlog item and/or inline TODO).

## Setup

Use this section to set up the local development environment before running the
commands below. This setup guidance should move to `README.md` after the port
is complete.

- Install Markdown link checker (one-time):

  ```sh
  cargo install lychee --version 0.24.1 --locked
  ```

- Install coverage runner (one-time):

  ```sh
  cargo install cargo-llvm-cov --locked
  ```

- If installed via asdf and not found on `PATH`, refresh shims:

  ```sh
  asdf reshim rust
  ```

## Common Commands

## Current Ruby project commands

- Run tests:

  ```sh
  bundle exec rake
  ```

- Run RuboCop:

  ```sh
  bundle exec rubocop
  ```

## Rust project commands (without wrapper scripts)

- Run tests:

  ```sh
  cd rust && cargo test
  ```

- Run coverage summary:

  ```sh
  cd rust && cargo llvm-cov --workspace --all-targets --summary-only
  ```

- Run lint checks:

  ```sh
  cd rust && cargo clippy --all-targets -- -D warnings
  ```

- Run build checks:

  ```sh
  cd rust && cargo build
  ```

- Run formatting checks:

  ```sh
  cd rust && cargo fmt -- --check
  ```

- Apply formatting:

  ```sh
  cd rust && cargo fmt
  ```

- Run Markdown link checks:

  ```sh
  find . -type f -name '*.md' \
    -not -path './.git/*' \
    -not -path './vendor/*' \
    -not -path './rust/target/*' \
    -not -path './coverage/*' \
    -not -path './tmp/*' \
    -print0 | xargs -0 lychee --offline --no-progress
  ```

## Verification wrapper scripts

Use these wrapper scripts for agent verification from the repository root:

- `./scripts/run-tests.sh`
- `./scripts/run-coverage.sh`
- `./scripts/run-lint.sh`
- `./scripts/run-build.sh`
- `./scripts/run-format.sh`
- `./scripts/run-lint-md.sh`

Each script:

- Print command output to the terminal.
- Capture full output in `tmp/agent/*.log` files.
- Print exit code and concise summary at the end.

## Pull Request Expectations

- Keep changes focused and easy to review.
- Include tests for behavior changes.
- Update documentation when behavior, architecture, or workflow changes.
