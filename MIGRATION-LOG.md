# Migration Increment Log

This file is the historical log of completed Rust migration increments. Use it
as the durable record of what has already been completed and why it was done.

Entries should be simple bullet points in reverse chronological order.

Suggested entry shape:

- YYYY-MM-DD: <short title> - <what was completed and why it mattered>

## Entries

- 2026-05-01: Add coverage workflow and reporting contract - Added a Rust
  coverage wrapper script and documentation rules to report previous/current
  coverage after each increment; aligned Rust test function names to
  sentence-style imperative snake_case.
- 2026-05-01: Establish the library/CLI seam - Replaced the placeholder
  hello-world entrypoint with a typed library `run` entrypoint and error
  surface, while keeping CLI output behavior stable and delegating rendering to
  the CLI boundary.
