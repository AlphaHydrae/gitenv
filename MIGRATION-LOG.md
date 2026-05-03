# Migration Increment Log

This file is the historical log of completed Rust migration increments. Use it
as the durable record of what has already been completed.

Entries should be simple bullet points in reverse chronological order.

Suggested entry shape:

- YYYY-MM-DD: <short title> - <what is completed, in present tense>

## Entries

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
