#!/usr/bin/env bash
#
# Run Rust build and capture output to "tmp/agent/build_output.log".
#
# Usage:
#   scripts/run-build.sh      # run cargo build
#   scripts/run-build.sh --release  # pass build flags
#
# Output is written to: "tmp/agent/build_output.log".
# Exit code matches cargo build's exit code.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_WORKSPACE="$REPO_ROOT/rust"
OUTPUT_LOG="$REPO_ROOT/tmp/agent/build_output.log"

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

set +e
if [[ $# -eq 0 ]]; then
  echo "Running: cargo build" | tee "$OUTPUT_LOG"
  cargo build 2>&1 | tee -a "$OUTPUT_LOG"
else
  echo "Running: cargo build $*" | tee "$OUTPUT_LOG"
  cargo build "$@" 2>&1 | tee -a "$OUTPUT_LOG"
fi

EXIT_CODE=${PIPESTATUS[0]}
set -e

echo "" >> "$OUTPUT_LOG"
echo "Exit code: $EXIT_CODE" >> "$OUTPUT_LOG"

echo
echo "===== EXIT STATUS ====="
echo "Exit code: $EXIT_CODE"
echo
echo "===== LAST 30 LINES OF OUTPUT ====="
tail -30 "$OUTPUT_LOG"

exit "$EXIT_CODE"
