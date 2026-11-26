#!/usr/bin/env bash
# PostToolUse: format an edited .rs file with the pinned toolchain and rustfmt.toml.
set -euo pipefail
f=$(jq -r '.tool_input.file_path // empty')
[[ "$f" == *.rs && -f "$f" ]] || exit 0
case "$f" in */target/*) exit 0 ;; esac
cd "${CLAUDE_PROJECT_DIR:-.}"
rustfmt --quiet "$f" 2>&1 | head -20 >&2 || true
exit 0
