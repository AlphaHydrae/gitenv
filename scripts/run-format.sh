#!/usr/bin/env bash
#
# Run Rust formatting checks and capture output to "tmp/agent/format_output.log".
#
# Usage:
#   scripts/run-format.sh         # check formatting
#   scripts/run-format.sh --write # apply formatting changes
#
# Output is written to: "tmp/agent/format_output.log".
# Exit code matches cargo fmt's exit code.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_WORKSPACE="$REPO_ROOT/rust"
OUTPUT_LOG="$REPO_ROOT/tmp/agent/format_output.log"

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
if [[ "${1:-}" == "--write" ]]; then
  echo "Running: cargo fmt" | tee "$OUTPUT_LOG"
  cargo fmt 2>&1 | tee -a "$OUTPUT_LOG"
else
  echo "Running: cargo fmt -- --check" | tee "$OUTPUT_LOG"
  cargo fmt -- --check 2>&1 | tee -a "$OUTPUT_LOG"
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
