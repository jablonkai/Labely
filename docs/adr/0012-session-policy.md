# ADR-0012: Session policy for file roots and print gating

- Status: proposed
- Date: 2026-11-14

## Context
ADR-0005 limits agents to configured file roots and keeps printing off until the user allows
it. If the MCP layer checked tool arguments, steps inside `command.batch`, `$N` references
and new file-taking commands could slip past it.

## Decision
- **Where.** `labely_core::Policy {file_roots, allow_print}` lives on the `Session`.
  `Session::execute` refuses `print.*` when printing is off, so the check also covers batch
  steps. File commands resolve paths through `Ctx::check_path`. These are `doc.open`,
  `doc.save`, `element.add_image`, and `device.connect` with `fake:<dir>`, because a
  FakePrinter writes files.
- **Error.** The new error code is `forbidden`, with the roots in `accepted` and a hint.
- **Defaults.** The CLI is unrestricted. `labely mcp --headless` restricts to `--root` dirs
  (default: the current directory) and prints only with `--allow-print`. The studio runs agent
  calls under a restricted policy: `--agent-root` dirs (default: home), and printing only while
  "Allow agents to print" is ticked. User actions keep the unrestricted policy.
- **Paths.** Paths are canonicalised; paths for writing canonicalise their parent. `..` cannot
  escape a root.

## Consequences
- Every new command that touches files must call `check_path`; review checks this.
- The policy is not saved in documents or the journal.
