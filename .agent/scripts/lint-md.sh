#!/usr/bin/env bash
#
# Run Markdown link checks and capture output to
# "tmp/agent/markdown_lint_output.log".
#
# Usage:
#   .agent/scripts/lint-md.sh  # check all Markdown files with lychee
#
# Output is written to: "tmp/agent/markdown_lint_output.log".
# Exit code matches lychee's exit code.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT_LOG="$REPO_ROOT/tmp/agent/markdown_lint_output.log"

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

cd "$REPO_ROOT"

RUN_STARTED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Run started at (UTC): $RUN_STARTED_AT" | tee "$OUTPUT_LOG"

LYCHEE_BIN=""
if command -v lychee >/dev/null 2>&1; then
  LYCHEE_BIN="$(command -v lychee)"
elif command -v asdf >/dev/null 2>&1; then
  ASDF_RUST_ROOT="$(asdf where rust 2>/dev/null || true)"
  if [[ -n "$ASDF_RUST_ROOT" && -x "$ASDF_RUST_ROOT/bin/lychee" ]]; then
    LYCHEE_BIN="$ASDF_RUST_ROOT/bin/lychee"
  fi
fi

MARKDOWN_FILES=()
while IFS= read -r -d '' file; do
  MARKDOWN_FILES+=("$file")
done < <(
  find "$REPO_ROOT" -type f -name '*.md' \
    -not -path "$REPO_ROOT/.git/*" \
    -not -path "$REPO_ROOT/vendor/*" \
    -not -path "$REPO_ROOT/rust/target/*" \
    -not -path "$REPO_ROOT/coverage/*" \
    -not -path "$REPO_ROOT/tmp/*" \
    -print0
)

set +e
if [[ ${#MARKDOWN_FILES[@]} -eq 0 ]]; then
  echo "No Markdown files found." | tee -a "$OUTPUT_LOG"
  EXIT_CODE=0
elif [[ -z "$LYCHEE_BIN" ]]; then
  echo "lychee is not available. Install with: cargo install lychee --version 0.24.1 --locked" | tee -a "$OUTPUT_LOG"
  echo "If installed via asdf, run: asdf reshim rust" | tee -a "$OUTPUT_LOG"
  EXIT_CODE=127
else
  echo "Running: $LYCHEE_BIN --include-fragments --offline --no-progress <markdown files>" | tee -a "$OUTPUT_LOG"
  "$LYCHEE_BIN" --include-fragments --offline --no-progress "${MARKDOWN_FILES[@]}" 2>&1 | tee -a "$OUTPUT_LOG"
  EXIT_CODE=${PIPESTATUS[0]}
fi
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
