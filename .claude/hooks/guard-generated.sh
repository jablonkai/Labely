#!/usr/bin/env bash
# PreToolUse: refuse edits to build output and generated sources.
set -euo pipefail
f=$(jq -r '.tool_input.file_path // empty')
[[ -n "$f" ]] || exit 0
case "$f" in
  */target/*|*/.build/*|*/.swiftpm/*|*/.gradle/*|*/DerivedData/*|\
  */LabelyFFIFFI.xcframework/*|*/Sources/LabelyFFI/LabelyFFI.swift)
    echo "Blocked: $f is build output or generated; regenerate it (cargo xtask, see AGENTS.md) instead of editing." >&2
    exit 2 ;;
esac
exit 0
