# ADR-0004: Command registry, envelope, snapshot undo, atomic batch

- Status: proposed
- Date: 2026-11-14

## Context
The GUI, CLI, MCP and mobile apps must all share one way to change a document, and agents need
atomic multi-step edits. See prior-art.md §3–4.

## Decision
- **Command trait.** Typed `Params`/`Output` (serde + schemars 1.x, `deny_unknown_fields`) and
  `CommandMeta {label, menu, shortcut, kind, undoable, journal}`.
- **Registration.** An explicit registry list. No `inventory`, so the registry is safe for static
  and FFI linking.
- **Ids.** `area.verb_object`. Params are snake_case with units in the name (`x_mm`).
- **Single entry point.** `Session::execute(id, Value) -> Envelope`.
  - Success: `{ok, result, changed, created, warnings}`.
  - Failure: `{ok:false, error:{code, message, param, accepted, hint}}`.
  - Error codes are a stable snake_case set.
- **Versioning.** `API_VERSION` plus a schema-hash test.
- **Undo.**
  - Whole-document `Arc` snapshots, one per Edit command. Capped at 200 steps or a byte budget.
  - `coalesce` keys merge repeated edits such as slider drags.
  - Canvas gestures use begin/preview/commit.
  - A panic guard rolls back.
  - Rejected: inverse ops. Labels are tiny, so they add bugs without a benefit. Also rejected:
    per-frame diffing, because it bypasses commands.
- **Batch.** `command.batch` is atomic by default.
  - All step params are validated first, then the steps run, and any failure rolls back.
  - A successful batch is one undo step.
  - References: `$N.path`, `$last.path`, `$$` as escape.
  - Limits: 128 steps, 4 MiB of results.
  - Steps that cannot be rolled back are refused inside an atomic batch: `print.*`, `device.*`,
    `doc.open`, `doc.close` and nested batches.
- **Journal.** Journal entries `{id, params}` give CLI scripts and replay tests.

## Consequences
- Menus, shortcuts and MCP tool listings are generated from metadata.
- Every new feature starts with a command and a schema.
