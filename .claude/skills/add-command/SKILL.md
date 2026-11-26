---
name: add-command
description: Add a new command to Labely's core command registry (or change an existing command's params, output, kind or undo behaviour) end to end — the `Command` impl in crates/core/src/commands/, registration in registry.rs, `Ctx::check_path` for file access, undo and batch semantics, integration tests, the API_VERSION / schema-hash / insta snapshot flow, the docs/agents/tools.md catalogue and the studio menu label. Use this whenever the user wants a new document operation, query, device action or agent-callable feature in Labely, wants a new param or result field on an existing command, or a test fails with `api_version_matches_schema_hash` or a changed `command_list` snapshot — even if they only describe the feature ("let agents flip a label", "add an element.lock_aspect param"). Hungarian triggers too: "adj hozzá egy új commandot", "új parancs", "csinálj egy új commandot", "új paraméter a commandhoz", "legyen erre egy parancs", "elhasalt az api_version_matches_schema_hash teszt", "frissítsd a schema hash-t". Not for adding a printer model (add-printer) or UI text only (i18n-key).
---

# Add a command

Every document change in Labely is a command (ADR-0004). The studio, CLI (`labely run`),
MCP (`command_run`) and the mobile FFI engine (`Engine::execute`) all reach the same
`Registry::builtin()` through `Session::execute`, so a command added in core is available
everywhere at once, and its schema is the agent contract. That is why the steps below care
about schemas, rollback and pinned hashes more than about any one frontend.

## 1. Choose id, kind and home

- **Id**: `area.verb[_object]`, lowercase, digits and `_` only (checked by
  `ids_unique_and_well_formed` in registry.rs). Reuse an existing area: `doc`, `element`,
  `data`, `template`, `paper`, `export`, `render`, `device`, `print`, `job`, `history`, …
- **File**: put it in the area's file in `crates/core/src/commands/` (`data.rs`, `element.rs`, …).
  A new area needs a new file plus `pub mod <area>;` in `commands/mod.rs` and the
  `use crate::commands::{…}` list in `registry.rs`.
- **Kind** (`CommandMeta` constructors in `crates/core/src/command.rs`):
  - `CommandMeta::query` — reads only; no undo step, not journaled.
  - `CommandMeta::edit` — changes a document; journaled, one undo step. Use
    `.undoable(false)` for edits that should not be undone (`doc.activate` does this).
  - `CommandMeta::device` — talks to a printer; no undo, refused inside atomic batches.
  - `CommandMeta::ui` — view state only.
- **Params**: snake_case with units in the name (`width_mm`, `font_size_pt`). Always take
  `#[serde(default)] doc_id: Option<String>` when the command works on a document.

## 2. Write it (pattern from `variable.set` and `data.find_record`)

```rust
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FooParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Doc comments become the schema description agents read: state units and defaults.
    pub value: String,
    /// Column to search; every column when absent.
    #[serde(default)]
    pub column: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FooOutput {
    /// 1-based, as everywhere in the agent API.
    pub record: usize,
}

/// `area.foo`
#[derive(Debug)]
pub struct Foo;

impl Command for Foo {
    const ID: &'static str = "area.foo";
    type Params = FooParams;
    type Output = FooOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit("Foo", "Agent-facing description: what it does, units, defaults.")
    }

    fn run(ctx: &mut Ctx<'_>, p: FooParams) -> Result<FooOutput, CommandError> {
        if p.value.trim().is_empty() {
            return Err(CommandError::bad_params("value must not be empty").with_param("value"));
        }
        let doc = ctx.doc_mut(p.doc_id.as_deref())?; // ctx.doc / ctx.doc_arc for queries
        // … change `doc` …
        Ok(FooOutput { record: 1 })
    }
}
```

Why these details matter:
- `deny_unknown_fields` turns an agent's typo into a `bad_params` error with the param path
  instead of silently ignoring it.
- Errors are the agent's only feedback. Use `CommandError::{bad_params, not_found, disabled,
  internal}` with `.with_param(…)`, `.with_accepted(…)` (valid values or nearest ids, ≤ 50)
  and `.with_hint("run data.get …")` naming the next command to try. Messages stay English;
  frontends localise only the error *title* by code.
- Library code denies `unwrap`, `expect` and `panic` (lints). Return `CommandError::internal`
  instead. A panic is caught and rolled back by `Session::execute`, but it is still a bug.
- Use `ctx.warn(…)` for "done, but note this" (e.g. an ignored param).
- Override `fn enabled(ctx)` only when the command is unavailable in some state; its `Err`
  is reported as `disabled` and greys out the studio menu item.

## 3. File access: `Ctx::check_path` (ADR-0012)

Any command that reads or writes a file must resolve the path through the session policy
before touching the disk. Agents (MCP headless `--root`, studio agent calls) run under a
restricted policy, and this is the only place it is enforced — including inside batches.

```rust
let path = ctx.check_path("path", &p.path, false)?; // read (doc.open, data.attach_csv)
let path = ctx.check_path("path", &p.path, true)?;  // write (export.png, doc.save)
```

The first argument is the param name, so a refusal comes back as `forbidden` with `param`.
Call it before any other work, as `export.png` does.

## 4. Undo and batch semantics (ADR-0004)

- Undo is a whole-document `Arc` snapshot: mutate through `ctx.doc_mut` (copy on write) and
  never keep the old document yourself. No inverse operation is needed.
- Any `Err` from `run` restores the full session state, so you can mutate first and fail
  later without leaving a half-done edit.
- What is *not* rolled back is anything outside the session: files written, printer I/O.
  If the new command has such a side effect and changes session state, add it to
  `refused_in_atomic` in `crates/core/src/commands/meta.rs` (today: `print.*`, `device.*`,
  `history.*`, `doc.open`, `doc.close`, `command.batch`). Pure queries that write files
  (`export.*`, `doc.save`) are allowed in batches.
- A successful atomic batch collapses into one undo step per document; nothing to do per command.

## 5. Register

Add `Entry::of::<area::Foo>()` to `builtin_entries()` in `crates/core/src/registry.rs`, next
to its area (order = `command.list` order). For a studio menu item, chain
`.in_menu("<menu>", Some("Ctrl+Shift+X"))` or `.in_menu("<menu>", None)`; the menu must be one
of `MENUS` in `crates/studio/src/menu.rs` (`file edit insert arrange label data view print`).

## 6. Tests (`crates/core/tests/it/`)

Tests drive commands through `Session::execute` with testkit fakes. Add to the area's file
(`data.rs`, `elements.rs`, `file.rs`, …), or create one and add `mod <name>;` to
`tests/it/main.rs`. Helpers there: `ok`, `err`, `session_with_doc`, `add_text`.

Cover: the happy path, each error code and its `param`, and for edits that `history.undo`
restores the previous state. For file commands, assert `ErrorCode::Forbidden` under
`s.set_policy(Policy::restricted(vec![root]))`, as `policy_limits_files_and_printing` in
`phase2.rs` does.

```sh
cargo test -p labely-core --test it <test_name>
```

## 7. Schema hash and snapshot

A new or changed command changes `Registry::schema_hash()` (ids, kind, undoable and
schemas, with descriptions and titles stripped), so `api_version_matches_schema_hash` in
`crates/core/tests/it/schema.rs` fails. That pin exists so client breakage is a deliberate
decision. When it fails:

1. `cargo xtask schema-hash` prints `schema hash: <hex>`.
2. Bump `API_VERSION` in `crates/core/src/lib.rs` only if it was not already bumped since
   the last release (`git log -p -- crates/core/src/lib.rs` against the last `v*` tag; with
   no tag yet, ask the user). Most commands land without a bump, as `data.find_record`
   did; the release skill re-checks this before tagging.
3. Paste the version and hash into `PINNED` in `crates/core/tests/it/schema.rs`.
4. Refresh the `command_list` insta snapshot:
   `INSTA_UPDATE=always cargo test -p labely-core`, then review
   `crates/core/tests/it/snapshots/it__schema__command_list.snap` in `git diff`.

A description-only change updates the snapshot but not the hash.

## 8. Docs

`docs/agents/tools.md` is maintained by hand (nothing generates or checks it). Add one row
to the `## Commands (API version N)` table, next to its area:
`` | `area.foo` | edit | value (trimmed, exact), column (default: any) → record (1-based) | ``.
Update `N` in the heading if you bumped `API_VERSION`. A larger feature also gets an ADR
(`docs/adr/NNNN-*.md` plus its line in `docs/adr/README.md`) — use the new-adr skill.

## 9. Frontends

- **MCP, CLI, FFI**: nothing to do; they dispatch by id. Only add a curated MCP shortcut
  in `crates/mcp/src/tools.rs` `TOOLS` if the user asks for one.
- **Studio**: a menu command needs `cmd-<id with . and _ as ->` (e.g.
  `cmd-data-find-record`) in both `crates/i18n/locales/{en,hu}/studio.ftl`;
  `every_menu_command_has_a_label_in_both_languages` in menu.rs fails otherwise. Any other
  new UI text follows the i18n-key skill.
- **Android / iOS**: only if the feature should appear there; they call the command by id
  through the engine, and their strings follow the i18n-key skill.

## 10. Verify

```sh
cargo test -p labely-core
cargo test -p labely-studio        # if you touched a menu or studio UI
cargo xtask i18n-check             # if you added text
cargo xtask ci                     # before committing
```

Run cargo quietly and grep the output for `FAILED`, `panicked` and `error` rather than
reading the full log.
