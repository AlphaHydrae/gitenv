# Migration Increment Log

This file is the historical log of completed Rust migration increments. Use it
as the durable record of what has already been completed.

Entries should be simple bullet points in reverse chronological order.

Suggested entry shape:

- YYYY-MM-DD: <short title> - <what is completed, in present tense>

## Entries

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
