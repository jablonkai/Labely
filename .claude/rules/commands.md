---
paths:
  - "crates/core/src/commands/**"
---

# Engine commands

Every frontend (studio, CLI, MCP, Android, iOS) runs the same command registry, so a command
change reaches all of them at once. Follow the `add-command` skill for the full recipe.

- **Register it.** Every document change is a command in `crates/core/src/commands/`, added to
  `builtin_entries()` in `crates/core/src/registry.rs`. An unregistered command does not exist
  for any frontend.
- **File access (ADR-0012).** A command that reads or writes files must resolve every path
  through `Ctx::check_path` (`crates/core/src/command.rs`). The MCP layer does not check
  arguments, so skipping it lets agents escape their `--root` dirs, also inside `command.batch`.
- **No unwrap/expect/panic** in library code; return a `CommandError`. Error `message` stays
  stable English for agents (ADR-0009).
- **Schema changes.** Changing a command's params or result changes the schema hash and fails
  `api_version_matches_schema_hash` (`crates/core/tests/it/schema.rs`). Then:
  1. bump `API_VERSION` in `crates/core/src/lib.rs` (once per release cycle);
  2. paste the new hash into `PINNED` in `crates/core/tests/it/schema.rs`
     (`cargo xtask schema-hash` prints it);
  3. refresh the snapshot: `INSTA_UPDATE=always cargo test -p labely-core`, and review
     `crates/core/tests/it/snapshots/it__schema__command_list.snap`.
- **Docs.** Add or update the command in `docs/agents/tools.md` (catalogue) and, if useful, a
  batch in `docs/agents/batch-examples.md`.
- **Parity.** Note whether the studio and the phone shells expose it (ADR-0042,
  `docs/feature-matrix.md`).

Test: `cargo test -p labely-core`, then `cargo xtask ci`.
