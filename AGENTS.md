# Agents

This file contains agent-specific instructions for this repository.

## Quick-reference

When working on the codebase, consult these documents as needed:

- [README](./README.md)
- [Architecture & design decisions](./ARCHITECTURE.md)
- [Migration plan](./MIGRATION.md)
- [Migration increment log](./MIGRATION-LOG.md)
- [Migration increments (living plan)](./MIGRATION-INCREMENTS.md)
- [Shared expectations](./CONTRIBUTING.md#shared-expectations) — design alignment, documentation
- [Documentation expectations](./CONTRIBUTING.md#documentation-expectations) — comments, clarity
- [Boundaries](./CONTRIBUTING.md#boundaries) — domain modules emit data/events, CLI owns terminal output
- [Dependency injection principles](./CONTRIBUTING.md#dependency-injection-principles) — composition root, test doubles
- [Testing guidelines](./CONTRIBUTING.md#testing-guidelines) — behavior coverage, parity checks, logging checks
- [Verification checklist](./CONTRIBUTING.md#verification-checklist) — testing, linting, formatting steps
- [Common commands](./CONTRIBUTING.md#common-commands) — Ruby checks, Rust commands, and verification wrappers
- [**SKILL: Migration Workflow Rust**](./.agent/skills/migration-workflow-rust/SKILL.md) — incremental delivery, verification, coverage
- [**SKILL: Gitenv Documentation Management**](./.agent/skills/gitenv-documentation-management/SKILL.md) — keeping docs consistent

## ⚠️ EXECUTION AND VERIFICATION MANDATE ⚠️

**These rules are non-negotiable. Violating them is a critical failure. Apply
them to EVERY task, EVERY time.**

### 🚨 TOP PRIORITY — PLAN INTEGRITY, COMPLETION HONESTY, GUIDELINE DISCIPLINE

- **🔴 NEVER lose track of the agreed plan.** At the start of every response,
  recover the full agreed scope from the conversation and session memory before
  doing anything else.
- **🔴 NEVER report completion for only the latest subtasks.** Completion status
  must cover the ENTIRE agreed plan, including earlier items.
- **🔴 NEVER claim a task is complete if any agreed item is still pending.** If
  work remains, explicitly say it is incomplete and list what is left.
- **🔴 GUIDELINES MUST BE APPLIED DURING CHANGES, NOT AFTER.** Keep the relevant
  contribution guideline section open while editing and review the exact rule
  before each change.
- **🔴 PROACTIVELY SURFACE MAINTAINABILITY DEBT.** Do not wait for a human to
  ask about readability issues. If a file grows large, mixes multiple concerns,
  or lacks required explanatory documentation, call it out immediately and
  propose either (a) a scoped increment now or (b) a clearly recorded backlog
  item.
- **🔴 READABILITY/DOCUMENTATION GATE IS MANDATORY FOR RUST INCREMENTS.** During
  each increment review, explicitly check whether changed files still have
  coherent responsibility boundaries and sufficient comments per [Documentation
  expectations](./CONTRIBUTING.md#documentation-expectations). If not, stop and
  either include the cleanup in scope or record the deferred cleanup in
  [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md) before claiming
  completion.
- **🔴 GUIDELINES ARE DOUBLY IMPORTANT FOR TEST CODE.** Test files are
  behavioral documentation for humans and agents; violating test guidelines is a
  critical quality failure.
- **🔴 TEST-NAMING GATE IS MANDATORY BEFORE ANY TEST EDIT.** Before writing or
  changing any test code, re-read [test code
  guidelines](./CONTRIBUTING.md#testing-guidelines) and [test naming
  style](./CONTRIBUTING.md#test-naming-style), then explicitly self-check each
  new/edited test name against those rules. Do not proceed if any test name
  violates them.
- **🔴 TEST-ASSERTION GATE IS MANDATORY BEFORE ANY TEST EDIT.** Before
  finalizing a test change, verify that each test fulfills [assertion quality
  criteria](./CONTRIBUTING.md#assertion-quality). Fix any mismatches before
  claiming completion. Ask for guidance if necessary.

### 🚨 VERIFICATION — STILL MANDATORY TOP PRIORITY

- **🔴 NEVER claim a task is complete without running the required checks.**
  Do not say "tests pass" without showing the actual command output and exit
  code. This is a critical recurring failure mode.
  - For migration increments, capture a **coverage baseline before code
    changes** by running `./.agent/scripts/coverage.sh` first and recording the
    current total line coverage.
  - Always run `./.agent/scripts/tests.sh` for test changes.
  - Always run `./.agent/scripts/lint.sh` for source changes.
  - Always run `./.agent/scripts/lint-md.sh` for documentation changes.
  - Always run `./.agent/scripts/build.sh` to verify compilation.
  - Always run `./.agent/scripts/format.sh` in write mode for formatting
    verification.
  - Use `./.agent/scripts/format.sh --check` only when explicitly requested by
    a human or when diagnosing formatting-only issues without applying changes.
  - For migration increments, always run `./.agent/scripts/coverage.sh` again
    after changes and report both previous and current coverage.
  - If coverage decreases significantly, **do not consider the increment
    complete.** Restore coverage before claiming completion. A coverage drop
    caused by new code in the current increment means the tests for that code
    are missing — adding those tests is part of the increment, not a separate
    follow-up. Treat a drop of about 0.25 percentage points or more as
    significant unless there is a stronger project-specific reason.
  - There are two narrow situations where scheduling coverage recovery in a
    separate increment is acceptable: (1) the increment is an intentionally
    incomplete intermediate step toward a larger feature, and the missing tests
    fit better with the next already-planned increment; or (2) restoring
    coverage requires significant architectural changes that cannot fit in the
    current increment — in that case, explicitly surface the issue and ask the
    human whether to fix it immediately, skip it, or schedule a new increment.
    In all other cases, restore coverage in the same increment.
  - When coverage cannot be restored immediately, document the specific
    uncovered lines and the reason in
    [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md) and/or inline TODO
    comments.
  - When wrappers are not applicable for a task, run interim checks documented
    in [CONTRIBUTING.md](./CONTRIBUTING.md#common-commands).
  - Show full command output including exit code, test count, or failure details.
- **🔴 DO NOT make claims about code correctness, style compliance, or guideline
  adherence without evidence.** Every single claim must map to:
  - A file you have read (with line numbers cited)
  - Command output you have captured (with exit code)
  - A before/after comparison you have performed
- **🔴 NEVER skip validation steps.** If guidelines say to test something, test
  it. If you changed it, validate it. No exceptions.

### 📋 TASK TRACKING — MANDATORY DURING EXECUTION

- **Maintain an active task list throughout the session using the
  `manage_todo_list` tool.**
  - Update status BEFORE starting each section (`in-progress`)
  - Update status IMMEDIATELY after finishing each section (`completed`)
  - DO NOT batch completions. Update one at a time.
- **Never assume a task is in progress or done.** Update the list before and
  after EACH piece of work, no exceptions.

### ✅ EVIDENCE REQUIREMENT — FOR EVERY CLAIM

- When you say "code is correct": Show the file and line numbers.
- When you say "tests pass": Show the command output and exit code.
- When you say "it matches guidelines": Show the specific guideline text and the
  file change that satisfies it.
- **DO NOT cite files you haven't actually read or show output from commands you
  haven't actually run.**

### 💾 SESSION STATE — CHECK AND MAINTAIN

- **At the start of every response, check for a session memory plan file** and
  read it.
- Maintain the task list in session memory throughout the work.
- Before claiming completion, review the original request + the current task
  list + the evidence (test output, changes made) all together.

---

## Working with humans

- Never make changes with Git. A human handles Git operations.
- Explain your reasoning and tradeoffs when making changes or suggestions.
- Ask a human when you are unsure or when the next action is not covered by the
  documented guidance.
- If requirements are ambiguous, contradictory, or likely to cause unintended
  behavior, stop, explain the concern, and resolve the ambiguity before
  implementation.
- If you detect edits you did not make in files you are about to modify, call
  it out and confirm intent before proceeding.
- When you notice changes you did not make, do not revert them. Explicitly call
  them out to the human and ask whether they are intentional before proceeding
  with related edits.
- Re-read files immediately before editing.
- Humans may edit files in parallel while the agent is working, including files
  the agent just changed and unrelated files in the same workspace.
- If a task requires additional files or changes beyond the explicit request,
  ask before creating them.
- When additional packages are needed, propose them first with a short summary
  of what each package does, its maintainability outlook, and its security risk
  profile. Never install packages yourself; let a human review and install them.
- When told that you made a mistake or that something was unacceptable, read
  the relevant contribution guidelines and agent instructions to determine what
  caused the failure. Then suggest specific documentation improvements — new
  rules, clarifications, or examples — that would help prevent the same mistake
  in future sessions. Do not simply acknowledge the error; diagnose its root
  cause and propose a fix to the guidance.

## Incremental delivery workflow for this project

- The project owner is not an experienced Rust programmer. Build the project in
  step-by-step, reviewable increments.
- Keep increments small enough to review comfortably. Preferred size is usually
  10-20 lines plus associated tests when practical.
- It is acceptable to implement multiple incomplete increments on the path to a
  full feature, as long as each increment remains reviewable and coherent.
- Every increment must compile and pass tests before being considered complete.
  If temporary code is required to preserve this property, add it and include a
  clear comment that it is temporary and will be removed in a later increment.
- Do not hesitate to add TODO comments when functionality is intentionally
  deferred to future increments.
- Explain Rust-specific features, idioms, and gotchas used in generated code,
  with extra clarity for someone coming mostly from Ruby, Java, JavaScript, and
  TypeScript.
- Preserve public behavior and API in each increment unless the behavior change
  is explicitly documented and approved for that increment.
- Before each increment, include a short note explaining why the increment
  exists and what review outcome it targets.
- Prefer additive changes first and cleanup/removal in a following increment,
  unless the cleanup is trivial and local.
- For each deferred concern, keep one clear TODO comment that includes a
  removal/closure condition.
- When introducing Rust concepts, include a brief translation for contributors
  coming from Ruby/Java/JavaScript/TypeScript.
- Add an increment-level regression test whenever a bug or edge case is
  discovered.
- Do not mix architectural refactors and feature delivery in the same
  increment unless explicitly approved.
- Keep [`MIGRATION.md`](./MIGRATION.md) focused on long-term migration strategy,
  scope, and phases.
- Maintain increment history in [`MIGRATION-LOG.md`](./MIGRATION-LOG.md)
  (historical record only). Keep entries concise and ordered so completed work
  remains easy to scan.
- For migration log entries, describe what changed. Do not add explicit
  coverage-outcome narration when the entry already states the added tests or
  behavior work.
- Write migration log entries in present tense.
- Maintain the active next-step backlog in
  [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md) (living plan only).
  Keep it focused on immediate agreed increments.
- When an increment is completed, immediately do both:
  1. Add or update its completed entry in
     [`MIGRATION-LOG.md`](./MIGRATION-LOG.md).
  2. Remove or rewrite its item in
     [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md).
- When an increment is completed, provide a suggested commit message in the
  conversation that is easy to copy for human review:
  1. Present the suggested commit message in a fenced code block so it is
     directly copyable.
  2. Start with an imperative title line that states what changed.
  3. Keep the title line at or under 72 characters.
  4. Do not use the title to explain why the change was done.
  5. Follow with a short body describing what changed.
  6. For commits that include multiple meaningful changes, you may use `-`
     bullet points in the body when that improves readability.
  7. Focus the body on product/code behavior changes, not expected maintenance
     steps.
  8. Do not mention routine test additions for a feature change when the body
     already states the feature/behavior change. Mention tests only when tests
     are the primary deliverable, or when the change introduces new test
     utilities, test infrastructure, or testing tools.
  9. Do not mention routine migration log/backlog updates unless those
     documentation changes are themselves the primary deliverable.
  10. Do not mention normal repository-upkeep tasks in suggested commit
      messages when those tasks are evident from the diff.
- **🔴 COMMIT-MESSAGE SELF-CHECK IS MANDATORY.** Before posting a suggested
  commit message, explicitly re-check rules 1-10 above and ensure the body does
  not include routine test additions, migration backlog/log updates, or format/
  lint/build upkeep unless one of those is the primary deliverable.
- Be proactive with migration documentation upkeep. Do not wait for explicit
  user prompts: when work changes scope, sequencing, assumptions, or outcomes,
  update [`MIGRATION-LOG.md`](./MIGRATION-LOG.md) and/or
  [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md) in the same change or
  explicitly suggest the needed update.
- **When the backlog in [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md)
  has been fully worked through**, proactively suggest the next set of
  increments. Before doing so:
  1. Re-read [`MIGRATION.md`](./MIGRATION.md) to understand the overarching
     migration plan, phases, and exit criteria.
  2. Read the most recent entries in
     [`MIGRATION-LOG.md`](./MIGRATION-LOG.md) to understand what was completed,
     what decisions were made, and whether the overall plan assumptions still
     hold.
  3. Suggest updates to [`MIGRATION.md`](./MIGRATION.md) if phases are complete,
     decisions have shifted, or exit criteria have been met (mark work as
     completed, adapt phase scope, etc).
  4. With that context, suggest appropriately-sized increments for immediate
     next work only (do not attempt to plan the entire remaining migration).
     Keep each increment to 10–20 lines plus tests when practical, and ensure
     each increment has a coherent scope and clear review target.

## Agent-specific execution rules

- **Before making any architectural change, present the design and ask for
  approval.** Architectural changes include: introducing new patterns or
  abstractions, restructuring module boundaries, changing the shape of DI
  contracts, introducing new interfaces or types that govern how modules
  interact, and reorganizing the flow of data between layers. Present the
  options, explain the tradeoffs, and wait for explicit approval before writing
  any code. Do not treat an architectural suggestion as an implementation
  request.
- If a requested change conflicts with the documented guidance, point out the
  conflict and resolve it with the human before proceeding.
- After making changes, review the architecture document and relevant
  contribution guidelines to identify any mismatch between the change and the
  documented conventions. If you find any, explain the mismatch and propose a
  correction to the change or the documentation.
- After any documentation change in [`README.md`](./README.md),
  [`ARCHITECTURE.md`](./ARCHITECTURE.md),
  [`CONTRIBUTING.md`](./CONTRIBUTING.md), [`MIGRATION.md`](./MIGRATION.md),
  [`MIGRATION-LOG.md`](./MIGRATION-LOG.md),
  [`MIGRATION-INCREMENTS.md`](./MIGRATION-INCREMENTS.md), or
  [`AGENTS.md`](./AGENTS.md), review the full set of those files for consistency
  and update any affected file in the same change when needed.
- **Maintain skills in sync with guidelines and instructions.** Whenever you
  update [`AGENTS.md`](./AGENTS.md), [`CONTRIBUTING.md`](./CONTRIBUTING.md),
  [`MIGRATION.md`](./MIGRATION.md), or [`ARCHITECTURE.md`](./ARCHITECTURE.md),
  review the skills in `.agent/skills/` that are referenced by those changes
  (currently `migration-workflow-rust` and `gitenv-documentation-management`)
  and update any references or guidance that has become outdated. Do not
  duplicate rule text; instead, update links and adjust explanations. If a
  skill's guidance contradicts updated instructions, correct it before claiming
  the primary documentation change is complete.
- If instructions are incomplete, contradictory, or likely to cause an
  unintended result, stop, explain the problem, and propose a better option.
- For refactor work, stop extracting when remaining duplication is local and
  readable and further abstraction would hide intent more than it removes
  complexity. In that case, explicitly recommend ending the phase.
- Prefer existing language and library utilities over custom reimplementations
  when they provide the required behavior.
- Do not generate large shell or language-specific scripts to make changes.
  Prefer direct file edits.
- Small one-off scripts are acceptable only when they are clearly the simplest
  safe option. If a larger script seems necessary, ask for approval first.
- If you must create temporary files, place them in `tmp/agent/` and do not
  touch the `.keep` file there.

## Agent coding guidelines

- Prefer type-safe, explicit models at boundaries.
- Validate external/untrusted configuration input at boundaries.
- Prefer clear and deterministic behavior over clever abstractions.
- Prefer standard library and widely used stable libraries over custom
  implementations.
- Keep behavior exposed by both library and CLI unless an explicit design
  exception is documented.
- Ensure logging is thorough and configurable by log level when working on
  diagnostics or command execution paths.
- **🔴 Never use `expect`, `unwrap`, `panic!`, or `unreachable!` in
  implementation code without explicit human approval.** Every fallible path
  must propagate a typed `Result` or `Option` to the caller using `?`. If you
  believe a path is unreachable due to enforced invariants, document that
  reasoning and return a typed error anyway — do not panic. `expect` and
  `unwrap` are only acceptable in test code.
- Test code
  - Avoid re-testing behavior at a higher abstraction layer when that behavior
    is already fully covered at a lower layer and the higher layer adds no
    unique logic.
  - If complete assertions are not feasible without disproportionate complexity,
    stop and ask for guidance instead of defaulting to partial assertions.

## Agent verification helpers

Follow the verification expectations in [CONTRIBUTING.md](./CONTRIBUTING.md).
When running checks as an agent, use the repository helper scripts below.

These wrappers run the commands documented in
[CONTRIBUTING.md](./CONTRIBUTING.md#common-commands).

### Automated tests

- Always use `./.agent/scripts/tests.sh` from the repository root.
- The script captures full output to `tmp/agent/test_output.log` and prints the
  exit code and a summary tail to stdout.

### Test coverage

- Use `./.agent/scripts/coverage.sh` from the repository root for test coverage.
- The script captures full output to `tmp/agent/coverage_output.log`, prints the
  exit code, and prints the total line coverage when available.
- On a default run the script also writes a line-by-line annotated source report
  to `tmp/agent/coverage_annotated.log`. To list every uncovered line quickly:
  ```sh
  grep -E '^\s+[0-9]+\|\s+0\|' tmp/agent/coverage_annotated.log
  ```

### Linting

- Use `./.agent/scripts/lint.sh` from the repository root.
- The script captures full output to `tmp/agent/lint_output.log` and prints the
  exit code and a summary tail to stdout.

### Building

- Use `./.agent/scripts/build.sh` from the repository root when verifying
  builds.
- The script captures full output to `tmp/agent/build_output.log` and prints the
  exit code and a summary tail to stdout.

### Format wrapper

- Use `./.agent/scripts/format.sh` from the repository root for formatting
  checks.
- The script captures full output to `tmp/agent/format_output.log` and prints
  the exit code and a summary tail to stdout.

### Markdown lint wrapper

- Use `./.agent/scripts/lint-md.sh` from the repository root for Markdown
  linting.
- The script lints all Markdown files in the project, captures full output to
  `tmp/agent/markdown_lint_output.log`, and prints the exit code plus a summary
  tail to stdout.

## Former Commands

Ruby checks are:

```sh
bundle exec rake
bundle exec rubocop
```
