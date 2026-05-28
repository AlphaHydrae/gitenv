# Migration Increment Log

This file is the historical log of completed Rust migration increments. Use it
as the durable record of what has already been completed.

Entries should be simple bullet points in reverse chronological order.

Suggested entry shape:

- YYYY-MM-DD: <short title> - <what is completed, in present tense>

## Entries

- 2026-05-28: Cover apply success paths and planning helper gaps - Add
  apply-stage tests for symlink overwrite, symlink overwrite-with-backup,
  missing-copy creation, copy skip, and copy overwrite-with-backup success
  flows, add direct operation helper coverage for directory child listing and
  saturating depth decrement, add intent-stage invalid `select.include`
  validation coverage, and improve total line coverage from 99.70% to 99.71%.
- 2026-05-28: Add filesystem operation boundary tests for error path coverage -
  Extract filesystem mkdir and remove operations (remove_file, remove_dir)
  behind injectable DirectoryCreator and PathRemover trait boundaries in
  boundary.rs, implement both traits on RealBoundary with full error mapping,
  add closure-based FnDirectoryCreator and FnPathRemover test doubles for
  deterministic error simulation, update ApplyContext to hold all 4 boundaries
  (target_probe, symlink_creator, directory_creator, path_remover), refactor
  apply_symlink_operation() and apply_copy_operation() to use injected
  boundaries, update lib.rs composition root to inject all boundaries, add 8
  comprehensive boundary.rs trait tests covering success paths and
  permission-denied scenarios, add 3 error-path action tests covering lines 141,
  158, 188, improve boundary.rs coverage from 86.62% to 99.57%, and reach 99.63%
  overall line coverage with 37 missed lines (6 inline closure artifacts, 1
  defensive match arm).
- 2026-05-27: Add repository runtime override precedence from CLI and env - Add
  clap-backed `--repo`/`GITENV_REPO` parsing with value-source attribution,
  resolve repository precedence (`flag` > `env` > config) before planning,
  validate whitespace-only selected repository roots with typed errors,
  preserve composition-root and CLI-boundary separation by moving parsing
  helper logic into `cli.rs`, extend unit coverage for precedence/value-source
  behavior, and document repository override precedence in the Rust README.
- 2026-05-26: Add directory symlink source support in planning and apply - Add
  a dedicated symlink source readability requirement so operation planning
  accepts directory sources for symlink actions while preserving copy-file
  checks, add apply-stage unit coverage for create/skip/backup-overwrite
  directory symlink conflicts, and add an end-to-end apply integration scenario
  that verifies directory sources become symlink targets.
- 2026-05-26: Validate Rust README config examples with strict ID mapping - Add
  explicit `readme-config-id` markers for every Rust README YAML config
  example, enforce language tags for all README fenced code blocks, validate
  one-to-one ID parity between README examples and expectations in
  `rust/tests/readme_examples.rs`, and assert exact parsed `Config` equality
  for each example.
- 2026-05-26: Add selector include patterns with deterministic precedence -
  Extend select config and intent models with optional include glob patterns,
  apply include-before-exclude precedence during selector expansion, match
  include and exclude patterns against source-relative paths in direct and
  recursive modes, add representative include coverage, and document the
  selector semantics in the Rust README.
- 2026-05-26: Move selector glob parsing to intent planning - Add a validated
  selector exclude pattern representation in intent planning, validate
  source-level `select.exclude` patterns during intent derivation, pre-validate
  global selector excludes in `lib.rs` before operation planning context
  wiring, remove operation-stage glob parsing, and add intent-stage invalid
  pattern diagnostics coverage.
- 2026-05-26: Make select.exclude use glob patterns - Replace exact-name
  selector excludes with source-relative glob matching, apply selector type
  filtering before exclude matching, add direct and recursive glob coverage in
  operation planner tests, add invalid glob validation coverage, extend
  recursive apply integration coverage for glob excludes, and document glob
  semantics and precedence in the Rust README.
- 2026-05-14: Add existing-directories-only recursive select policy - Extend
  select config and intent models with `existing_directories_only`, keep
  recursive selector entries visible when their target directory chain is
  missing, render those entries as skipped in `info`, and skip their concrete
  filesystem work during `apply`.
- 2026-05-14: Implement recursive select operation expansion - Expand selector
  planning through one depth-parameterized directory traversal path.
- 2026-05-14: Add recursive select intent shape with default false - Extend
  select configuration and intent models with `recursive: bool` defaulting to
  `false`, add parser and intent tests for omitted vs explicit `recursive:
false` normalization and `recursive: true` propagation, and document deferred
  recursive operation traversal in operation expansion.
- 2026-05-14: Carry source availability on operation plans - Replace the
  operation-plan `actions` shape with ordered operation entries that preserve
  declaration order for concrete actions and planning issues, carry typed
  source availability through operation planning, surface unavailable/planning
  issue entries in `info` rendering, and block `apply` with grouped source
  diagnostics before filesystem mutation.
- 2026-05-13: Replace select dotfiles boolean with selector type enum - Replace
  `select.dotfiles` with `select.type` (`dot`, `non-dot`, `all`) across config
  parsing, intent and operation models, selector expansion logic, tests, and
  docs.
- 2026-05-12: Auto-exclude `.DS_Store` through operation context - Add a global
  exclusion list to `OperationContext`, resolve the platform-specific default in
  `lib.rs`, and keep selector expansion independent of OS logic while preserving
  configurable selector filtering.
- 2026-05-12: Add explicit config path precedence in CLI parsing - Add
  `--config`/`-c` parsing in `cli.rs` with clap-backed `GITENV_CONFIG` fallback,
  resolve and load the configuration once in `run_cli`, pass loaded config into
  `run_info`/`run_apply`, and keep default path resolution limited to XDG/HOME
  behavior.
- 2026-05-12: Composition-root wiring docs parity pass — Update ARCHITECTURE.md
  to document the composition root in `lib.rs`, the eight production entry
  points, the stage-owned context approach, and module boundaries with concise
  responsibility descriptions.
- 2026-05-12: Extract shared operation and apply test doubles — Move repeated
  closure-backed test doubles for directory listing, target probing, and
  symlink creation into `boundary::test_doubles`, then reuse them from
  `operation.rs` and `actions.rs` tests to reduce duplication while preserving
  existing test behavior.
- 2026-05-12: Readability cleanup and naming pass for DI architecture — Add
  module-level doc comments to `boundary.rs`, `operation.rs`, and `actions.rs`;
  improve function-level docs on the three composition-root entrypoints in
  `lib.rs`; extract a shared `MapEnvReader` test double into
  `boundary::test_doubles` and replace the ad-hoc `TestBoundary` in `lib.rs`
  tests with the shared double.
- 2026-05-12: Rebalance apply boundary — Convert apply execution from
  callback-style seams to an apply-stage context, wire real target/symlink
  adapters from the composition root, and keep command orchestration tests
  focused on staged fakes rather than lower-layer behavior.
- 2026-05-12: Rebalance operation boundary — Convert operation planning from
  callback-style seams to a context-backed approach, remove the SystemCalls
  bootstrap shim, and wire the operation stage through the shared boundary
  traits same as intent planning.
- 2026-05-12: Introduce intent stage context — Shift intent planning from
  callback-based wiring to a dedicated stage context, keep the public entrypoint
  centered in `lib.rs`, and align snapshot coverage with the internal
  context-driven planning flow.
- 2026-05-11: Introduce command-owned orchestration contexts - Replace `info`
  and `apply` command closure bundles with command-owned contexts that carry
  loaded config, resolved home directory, and wired stage entrypoints; move
  context construction into `lib.rs` composition-root helpers.
- 2026-05-11: Extract info and apply orchestration modules - Move command
  orchestration for `run_info` and `run_apply` into `app/info.rs` and
  `app/apply.rs`, keep `lib.rs` focused on composition-root wiring/default
  config resolution.
- 2026-05-11: Extract shared dependency boundary traits - Add a shared
  `boundary` module for environment/config/filesystem dependency traits, move
  concrete runtime adapters there, and wire intent/operation/apply composition
  through that boundary from `lib.rs`.
- 2026-05-10: Centralize runtime wiring in lib composition root - Move real
  runtime adapter wiring for intent, operation, and apply entrypoints into
  `lib.rs`; keep implementation modules focused on injectable behavior.
- 2026-05-09: Expand home-relative paths in guards and includes.
- 2026-05-09: Assert full directory state in action and integration tests - Add
  a reusable recursive directory snapshot helper for test fixtures, migrate
  integration tests to use full snapshots for apply outcomes and no-change
  checks.
- 2026-05-09: Rename ProgramError variants to failure-oriented names - Replace
  ambiguous operation-style `ProgramError` variant names with concise
  problem-oriented names across config loading, selector expansion, status
  inspection, and apply execution paths.
- 2026-05-09: Skip copy apply work when target content already matches - Reuse
  copy-status inspection before destructive copy apply work so overwrite and
  backup modes skip unchanged targets while still replacing mismatched files.
- 2026-05-09: Reject configs with no sources or no per-source config items
  during parsing.
- 2026-05-09: Resolve include paths relative to declaring config files - Resolve
  relative include paths against each declaring config file's directory, keep
  absolute and `~` include paths unchanged.
- 2026-05-09: Display home-based paths with tilde in CLI output - Render
  source/target paths under HOME with `~`, including `points to ...`
  diagnostics.
- 2026-05-09: Expand primary CLI end-to-end workflow coverage - Move the broad
  end-to-end workflow scenarios into `integration.rs`, keep `cli_output.rs`
  focused on minimal subprocess-boundary checks, broaden the info/apply
  scenarios to cover mixed symlink/copy/select states and conflicts, and add a
  temporary Rust-port README with section-by-section configuration guidance.
- 2026-05-09: Unify ANSI color policy - Introduce shared color-policy handling
  with `COLOR=auto|yes|no`, add a `--color` CLI override with precedence over
  environment values.
- 2026-05-09: Wire --log-level flag into CLI and library - Add a `--log-level`
  flag to the CLI (default: `warn`), implement a minimal dependency-free log
  subscriber in `logging.rs`. Expose `LogLevel` and `init_logging` in the public
  library API.
- 2026-05-09: Introduce structured logging across domain modules - Add the
  shared Rust logging module and `log` dependency, emit structured domain
  events from config loading/parsing, intent planning, operation expansion,
  status inspection, and apply execution, while keeping logging transport and
  level controls at the composition-root boundary.
- 2026-05-08: Move CLI rendering out of lib.rs into cli.rs - Move status/apply
  rendering helpers, ANSI color logic, and terminal-color probing into the CLI
  module; keep `lib.rs` focused on orchestration; migrate rendering tests to
  `cli.rs`; and add coverage for renderer branch handling.
- 2026-05-06: Inject system dependencies from the CLI composition root - Add a
  top-down system-calls dependency path from `run_cli` through config-path
  resolution, terminal-color decisions, and operation-plan HOME resolution;
  replace compile-time test color gating with deterministic injected behavior;
  and expand unit tests to cover deterministic env/terminal scenarios and
  dispatch wrappers.
- 2026-05-06: Add colorized status rendering coverage - Render primary
  inspection/apply status states with ANSI colors when output is a terminal and
  `NO_COLOR` is unset, while keeping deterministic plain rendering in
  non-terminal contexts and tests.
- 2026-05-06: Add explicit CLI command parsing for info and apply - Add clap
  dependency with derive macro support, introduce `info` and `apply`
  subcommands, implement command dispatch in main.rs, add render_apply_output to
  show operation outcomes.
- 2026-05-04: Expand status modeling for info and apply workflows - Generalize
  status inspection to return typed per-operation outcomes for both symlink and
  copy actions, compare copy source/target content via streamed hashes instead
  of loading full files into memory, extend default inspection rendering to
  support copy statuses, and keep apply outcomes aligned as structured
  per-operation results.
- 2026-05-03: Introduce copy apply execution with option parity - Execute
  copy operations in the apply path with `mkdir` and
  `skip`/`overwrite`/backup conflict semantics, return typed apply outcomes
  that preserve operation kind for both copy and symlink actions, and add
  Unix filesystem integration coverage for copy execution parity scenarios.
- 2026-05-03: Implement symlink conflict semantics - Extend symlink apply
  execution to honor `skip`/`overwrite`/backup conflict policies, create
  target parent directories when `mkdir` is enabled, and return deterministic
  typed errors for backup, remove, and directory-creation failures.
- 2026-05-03: Add a minimal symlink apply executor slice - Introduce an
  `actions` execution entrypoint that applies only missing-target symlink
  operations, reports structured per-operation outcomes for existing-target
  and copy branches.
- 2026-05-03: Add core intent/operation snapshot coverage - Add representative
  golden coverage that locks intent and operation planning outcomes for
  shorthand and canonical config parity, include ordering, guard evaluation,
  selector expansion, and item-level option overrides.
- 2026-05-03: Add config-path override and setup diagnostics - Support
  `GITENV_CONFIG` environment variable to override the default config path,
  add deterministic setup guidance to missing/invalid config errors that
  suggests where to create the config file or how to use `GITENV_CONFIG`.
- 2026-05-03: Wire the default CLI inspection flow - Replace the placeholder
  Rust CLI path with a thin config -> intent -> operation -> symlink-status
  inspection pipeline, resolve the default config location from XDG
  (`$XDG_CONFIG_HOME/gitenv/config.yml`) with
  `~/.config/gitenv/config.yml` fallback, render default no-arg status output
  from structured inspection data, and add CLI tests for default output and
  missing-config exit behavior.
- 2026-05-03: Add the first symlink status inspection slice - Introduce a
  dedicated Rust `status` module with structured symlink inspection results,
  add a typed entrypoint that inspects one concrete symlink operation without
  filesystem mutation.
- 2026-05-03: Add the first operation-plan expansion slice - Introduce a
  dedicated Rust `operation` module that derives flat concrete copy/symlink
  operations from `IntentPlan`, resolve repository-relative source paths and
  home-relative target paths, and expose operation-stage entrypoints in the
  public library API.
- 2026-05-03: Clarify intent-stage planner naming - Introduce intent-stage
  planner names (`IntentPlan`, `IntentSource`, `IntentAction`, and intent
  derivation entrypoints), remove legacy `ExecutionPlan`/`Planned*` naming and
  `derive_execution_plan*` entrypoints, and update planner tests to assert the
  intent entrypoints directly.
- 2026-05-03: Realign migration documents with current planning state - Update
  phase checklist status in `MIGRATION.md`, clarify that the current
  `ExecutionPlan` output is the intent-plan stage while operation-plan
  expansion is still pending, and resequence `MIGRATION-INCREMENTS.md` so the
  next increment first clarifies plan-stage naming before adding operation-plan
  expansion and then status/CLI slices.
- 2026-05-02: Move parser-focused tests into config.rs and document config API -
  Relocate parse/load/guard/include/source-root/item-override parsing tests from
  `rust/src/plan.rs` into `rust/src/config.rs` so parser behavior is tested
  alongside parser code; keep planning-behavior tests in `plan.rs`. Add doc
  comments to non-obvious config public models and parsing functions.
- 2026-05-02: Add doc comments to plan.rs public types and functions - Add
  concise doc comments to all public types (`ConflictPolicy`, `ExecutionPlan`,
  `PlannedSource`, `PlannedAction`, `PlannedFileAction`, `PlannedSelectAction`)
  and public entry-point functions (`derive_execution_plan`,
  `derive_execution_plan_with_env`).
- 2026-05-02: Review and fix plan.rs test quality - Rename three tests that
  violated guidelines (`reject_missing_required_includes`,
  `each_included_configs_defaults_apply_only_to_its_own_sources`,
  `parse_error_in_included_config_propagates_immediately`); add partial-assertion
  justification comment to `resolve_env_backed_include_paths_from_environment`.
- 2026-05-02: Move planning and include tests into the plan module - Migrate all
  planning, source-root, guard, include-parsing, include-planning, and
  item-override tests from `rust/src/lib.rs` into `rust/src/plan.rs`; reduce
  `rust/src/lib.rs` to a single `show_the_default_message` test; coverage
  stays at 98.13%.
- 2026-05-02: Move core config tests into the config module - Migrate default,
  canonical-shape, load-from-file, and shorthand/normalization parsing tests
  from `rust/src/lib.rs` into `rust/src/config.rs` so config behavior coverage
  lives next to parsing/model code while preserving test behavior and coverage.
- 2026-05-02: Split config and planner code into focused modules - Move
  configuration model/parsing into `rust/src/config.rs` and planning
  derivation/types into `rust/src/plan.rs`, keep `rust/src/lib.rs` as a thin
  public re-export surface, and preserve existing behavior and API shape.
- 2026-05-02: Support per-config-item option overrides - Add optional `mode`,
  `to`, `mkdir`, `overwrite`, and `backup_on_overwrite` override fields to
  `FileConfig` and `SelectConfig`; introduce `ResolvedOptions` to hold shared
  resolved execution options and refactor `PlannedFileAction` and
  `PlannedSelectAction` to use it; add `resolve_item_options` helper that merges
  item-level overrides on top of inherited defaults and rejects the explicit
  `overwrite: false` + `backup_on_overwrite: true` combination at item level.
- 2026-05-02: Recover include-planner coverage and diagnostics workflow - Add
  focused include parser/planner tests for bare-dollar source/include
  shorthand, explicit-path planning, backup-on-overwrite conflict behavior, and
  non-read include error propagation; simplify include test injectables to
  avoid dead closure branches; and extend coverage tooling/docs to emit an
  annotated coverage log for fast uncovered-line inspection.
- 2026-05-02: Add deterministic config includes - Add declarative YAML
  `includes` support with deterministic include ordering (including file
  sources before included sources), per-config default isolation, include
  de-duplication, cycle detection, and stable include diagnostics for missing
  files and missing environment-backed include variables.
- 2026-05-02: Add declarative filesystem guards - Add source-level destination
  overrides and declarative `when` guards (`to_exists` and `directory_exists`),
  evaluate guard outcomes deterministically during planning without executing
  config code.
- 2026-05-02: Interpret dollar-prefixed source roots as environment bindings -
  Normalize shorthand source roots that begin with `$` into required
  environment-backed source roots and add an explicit path object variant so
  literal paths that start with `$` remain supported.
- 2026-05-02: Add environment-backed config values - Accept explicit required
  environment declarations and environment-backed source roots, resolve those
  bindings during planning, and reject missing variables with stable
  diagnostics.
- 2026-05-02: Add a deterministic execution plan model - Derive structured,
  side-effect-free execution plans from normalized config and keep plan output
  deterministic across equivalent shorthand and canonical inputs.
- 2026-05-02: Normalize shorthand into the canonical model - Accept shorthand
  file config entries and normalize them to canonical file config items.
- 2026-05-02: Reject unknown keys in config parsing - Enforce strict serde
  unknown-field rejection for top-level and nested config objects and add
  parser tests for unknown top-level and defaults keys.
- 2026-05-02: Allow omitted defaults in configuration parsing - Apply
  serde-driven defaulting so configs parse when `defaults` is missing or
  partially specified, and assert omitted values are filled from
  `Defaults::default()`.
- 2026-05-02: Improve parser behavior coverage for select and file-read paths -
  Add targeted Rust tests for select-item parsing and missing-file read
  failures, and tighten invalid-config assertions so parser error behavior is
  fully exercised.
- 2026-05-01: Parse the smallest valid YAML config - Add `serde_yaml` parsing
  for the canonical config model, including a file-loading boundary and clear
  invalid-configuration errors for malformed YAML and missing required fields.
- 2026-05-01: Define the canonical config model - Add canonical configuration
  types in the Rust library (`Config`, `Defaults`, `Source`, and config item
  variants) with explicit owned fields and default action semantics to lock the
  internal vocabulary before parser work.
- 2026-05-01: Add coverage workflow and reporting contract - Add a Rust
  coverage wrapper script and documentation rules to report previous/current
  coverage after each increment; align Rust test function names to
  sentence-style imperative snake_case.
- 2026-05-01: Establish the library/CLI seam - Replace the placeholder
  hello-world entrypoint with a typed library `run` entrypoint and error
  surface, while keeping CLI output behavior stable and delegating rendering to
  the CLI boundary.
