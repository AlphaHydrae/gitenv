# Contributing

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
- Keep CI checks aligned with local verification expectations. When required
  checks or wrapper scripts change, update `.github/workflows/rust.yml` and
  `Makefile` in the same change.

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

## Coding Guidelines

- **DO NOT** use `expect`, `unwrap`, `panic!`, or `unreachable!` in
  implementation code (non-test code). Every failure path must be represented as
  a typed `Result` or `Option` and propagated to the caller. Use `?` for
  propagation. If a path is genuinely unreachable due to invariants already
  enforced elsewhere, document why and return a typed error instead of
  panicking.
- `expect` and `unwrap` are acceptable in test code only, where a panic
  produces a clear failure at the test site.

## Testing Guidelines

- Cover edge cases and failure modes explicitly.
- Avoid duplicate behavior coverage across layers when a lower-layer test
  already proves the behavior and the higher layer adds no new transformation
  or decision logic.
- Coverage target is 100%; if lower, document uncovered paths and rationale.
- Treat coverage as a signal, not a substitute for assertion quality.
- Include parity tests for equivalent library and CLI workflows.
- Include logging tests for level filtering and expected diagnostic visibility.
- Keep `rust/README.md` config examples aligned with
  `rust/tests/readme_examples.rs`. That test enforces language tags on fenced
  code blocks and one-to-one example ID parity; update
  `expected_readme_configs` when README examples intentionally change.
- **Prefer unit tests over integration tests.** Unit tests (in `#[cfg(test)] mod
tests` within source files) are faster, more precise, and easier to maintain.
  Use integration tests in `tests/` only for behavior that genuinely requires
  running the compiled binary as a subprocess — exit codes, the stderr/stdout
  boundary, and binary linking. Output content and command dispatch logic belong
  in unit tests.

### Test naming style

- Describe the observable behavior and outcomes in plain human-readable
  language.
- Use sentence-style `snake_case`. Prefer an imperative verb at the start (for
  example: `show_the_default_message_on_stdout`), but declarative sentences are
  acceptable when they more clearly describe the expected behavior (for example:
  `the_default_action_is_a_home_symlink`). Avoid third-person singular forms
  such as `shows_...`.
- Keep test names human-readable and avoid technical details that do not help
  explain the expected behavior (e.g. method name, return value, implementation
  details). Put necessary technical context in inline comments instead.
- Keep names focused on externally observable behavior and avoid implementation
  terms such as `returns`, `checks`, `calls`.
- Do not include implementation technology in test names when it does not change
  the expected behavior (for example, avoid format labels such as `yaml` in
  config behavior tests, since we only parse YAML configs anyway).
- Avoid redundant wording in test names (for example, do not pair `missing`
  with `required` when one already implies the other).
- Use `cannot` for constraints and failure-mode examples.

### Assertion quality

- Test clear, externally observable outcomes rather than implementation details.
- Make assertions complete for the behavior under test. When a function returns
  a full structured result that can be asserted directly, assert that full
  result rather than a partial subset.
- If complete assertions become too long or repetitive, extract test helper
  constructors/builders to keep tests readable while still asserting full
  outcomes.
- If complete assertions are not feasible without disproportionate complexity,
  stop and ask for guidance instead of defaulting to partial assertions.
- If complete assertions are intentionally not used, include a brief comment
  explaining the reason.
- In domain tests, assert structured return values and emitted events instead of
  asserting terminal output side effects.
- In CLI-layer tests, assert output behavior at the CLI boundary only
  (`stdout`, `stderr`, log level gating, and rendering decisions).
- Keep parseable `stdout` contracts stable. When adding output modes, include
  tests that verify `stderr` diagnostics do not corrupt parseable `stdout`.

## Verification Checklist

Apply checks that match your change scope.

0. For migration increments, run coverage before making code changes to capture
   the baseline used to detect regressions.
1. Ensure behavior is correct to the best of your ability.
2. Run relevant tests.
3. Run linting checks.
4. Run build checks.
5. Run documentation lint checks when Markdown files changed.
6. Run formatting.
7. Run coverage for code changes. Avoid significant coverage drops.

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
  ./scripts/lint-md
  ```

## Verification wrapper scripts

Use these wrapper scripts for agent verification from the repository root:

- `./.agent/scripts/tests.sh`
- `./.agent/scripts/coverage.sh`
- `./.agent/scripts/lint.sh`
- `./.agent/scripts/build.sh`
- `./.agent/scripts/format.sh`
- `./.agent/scripts/lint-md.sh`

Each script:

- Prints command output to the terminal.
- Captures full output in `tmp/agent/*.log` files.
- Prints the exit code and a concise summary at the end.

`./.agent/scripts/coverage.sh` also writes a line-by-line annotated report to
`tmp/agent/coverage_annotated.log` on default runs. To list uncovered lines:

```sh
grep -E '^\s+[0-9]+\|\s+0\|' tmp/agent/coverage_annotated.log
```

## Pull Request Expectations

- Keep changes focused and easy to review.
- Include tests for behavior changes.
- Update documentation when behavior, architecture, or workflow changes.
