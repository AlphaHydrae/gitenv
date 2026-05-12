#!/usr/bin/env bash
#
# Run Rust tests and capture output to "tmp/agent/test_output.log".
#
# Usage:
#   .agent/scripts/tests.sh                    # run all tests
#   .agent/scripts/tests.sh --test cli_output  # pass through cargo test args
#
# Output is written to: "tmp/agent/test_output.log".
# Exit code matches cargo test's exit code.

set -euo pipefail

# Resolve repository root (script is in .agent/scripts/ subdirectory)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_WORKSPACE="$REPO_ROOT/rust"
OUTPUT_LOG="$REPO_ROOT/tmp/agent/test_output.log"

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

set +e
if [[ $# -eq 0 ]]; then
  echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"
  echo "Running: cargo test" | tee -a "$OUTPUT_LOG"
  cargo test 2>&1 | tee -a "$OUTPUT_LOG"
else
  echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"
  echo "Running: cargo test $*" | tee -a "$OUTPUT_LOG"
  cargo test "$@" 2>&1 | tee -a "$OUTPUT_LOG"
fi

EXIT_CODE=${PIPESTATUS[0]}
set -e

echo "" >> "$OUTPUT_LOG"
echo "Exit code: $EXIT_CODE" >> "$OUTPUT_LOG"
RUN_COMPLETED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Run completed at (UTC): $RUN_COMPLETED_AT" >> "$OUTPUT_LOG"

echo
echo "===== EXIT STATUS ====="
echo "Exit code: $EXIT_CODE"
echo "Run started at (UTC): $RUN_STARTED_AT"
echo "Run completed at (UTC): $RUN_COMPLETED_AT"
echo
echo "===== LAST 30 LINES OF OUTPUT ====="
tail -30 "$OUTPUT_LOG"

exit "$EXIT_CODE"
