#!/usr/bin/env bash
#
# Run Rust coverage and capture output to "tmp/agent/coverage_output.log".
#
# Usage:
#   .agent/scripts/coverage.sh         # run coverage summary + annotated text report
#   .agent/scripts/coverage.sh --html  # pass through cargo llvm-cov args
#
# Output files written to tmp/agent/:
#   coverage_output.log      — summary table (Regions / Functions / Lines)
#   coverage_annotated.log   — line-by-line annotated source (always written on
#                              a default run, omitted when args are passed)
#
# Finding uncovered lines (execution count = 0):
#   grep -E '^\s+[0-9]+\|\s+0\|' tmp/agent/coverage_annotated.log
#
# Exit code matches cargo llvm-cov's exit code.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_WORKSPACE="$REPO_ROOT/rust"
OUTPUT_LOG="$REPO_ROOT/tmp/agent/coverage_output.log"
ANNOTATED_LOG="$REPO_ROOT/tmp/agent/coverage_annotated.log"

mkdir -p "$REPO_ROOT/tmp/agent"

# Add asdf bin + shims to PATH if available (non-interactive shells may miss this).
if [[ -d "/opt/homebrew/bin" ]]; then
  export PATH="/opt/homebrew/bin:$PATH"
fi

if [[ -d "$HOME/.asdf/shims" ]]; then
  export PATH="$HOME/.asdf/shims:$PATH"
fi

if [[ -f "$HOME/.asdf/asdf.sh" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.asdf/asdf.sh"
fi

cd "$RUST_WORKSPACE"

RUN_STARTED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"
  {
    echo "cargo-llvm-cov is not available."
    echo "Install with: cargo install cargo-llvm-cov --locked"
    echo "If installed via asdf, run: asdf reshim rust"
  } | tee -a "$OUTPUT_LOG"

  echo
  echo "===== EXIT STATUS ====="
  echo "Exit code: 127"
  echo
  echo "===== LAST 30 LINES OF OUTPUT ====="
  tail -30 "$OUTPUT_LOG"
  exit 127
fi

set +e
if [[ $# -eq 0 ]]; then
  echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"
  echo "Running: cargo llvm-cov --workspace --all-targets --summary-only --fail-under-lines 100" | tee -a "$OUTPUT_LOG"
  cargo llvm-cov --workspace --all-targets --summary-only --fail-under-lines 100 2>&1 | tee -a "$OUTPUT_LOG"
  SUMMARY_EXIT_CODE=${PIPESTATUS[0]}

  # Also write a line-by-line annotated report so that uncovered lines can be
  # surfaced quickly with:
  #   grep -E '^\s+[0-9]+\|\s+0\|' tmp/agent/coverage_annotated.log
  echo "Running: cargo llvm-cov --workspace --all-targets --text (annotated)" >> "$OUTPUT_LOG"
  cargo llvm-cov --workspace --all-targets --text 2>/dev/null > "$ANNOTATED_LOG" || true

  EXIT_CODE=$SUMMARY_EXIT_CODE
else
  echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"
  echo "Running: cargo llvm-cov $*" | tee -a "$OUTPUT_LOG"
  cargo llvm-cov "$@" 2>&1 | tee -a "$OUTPUT_LOG"
  EXIT_CODE=${PIPESTATUS[0]}
fi
set -e

echo "" >> "$OUTPUT_LOG"
echo "Exit code: $EXIT_CODE" >> "$OUTPUT_LOG"
RUN_COMPLETED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Run completed at (UTC): $RUN_COMPLETED_AT" >> "$OUTPUT_LOG"

TOTAL_LINE="$(grep -E '^TOTAL' "$OUTPUT_LOG" | tail -1 || true)"
TOTAL_PERCENT=""
if [[ -n "$TOTAL_LINE" ]]; then
  TOTAL_PERCENT="$(echo "$TOTAL_LINE" | awk '{print $(NF-3)}')"
fi

echo
echo "===== EXIT STATUS ====="
echo "Exit code: $EXIT_CODE"
if [[ -n "$TOTAL_PERCENT" ]]; then
  echo "Total line coverage: $TOTAL_PERCENT"
fi
echo "Run started at (UTC): $RUN_STARTED_AT"
echo "Run completed at (UTC): $RUN_COMPLETED_AT"
echo
echo "===== LAST 30 LINES OF OUTPUT ====="
tail -30 "$OUTPUT_LOG"

exit "$EXIT_CODE"
