# Prior art: ArtCraft Crafting Apps and ZigZag

What two families of open-source Rust creative tools do about the problems Labely also has to
solve: one command surface shared by the GUI, CLI and MCP, undo, an MCP server inside an egui
app, and talking to a hardware device. Everything here comes from reading the source at the
commits below. No code was built or run. Patterns are paraphrased, and only short identifiers
are quoted.

## Sources

| Key | Repository | Commit read | Licence |
|---|---|---|---|
| PC | github.com/storytold/photocraft | `629751503165118d5fec5f70e64fe2d6592f7b53` | MIT OR Apache-2.0 |
| VC | github.com/storytold/vectorcraft | `bada8e17103db9917de44e40a1cead5a7fc71c3c` | MIT OR Apache-2.0 |
| DC | github.com/storytold/designcraft | `8b169f8ee7832e60ed1ac4c9f5dd34f954d3a992` | MIT OR Apache-2.0 |
| CL | github.com/storytold/craft-libs | `75a654f4f83d0aa0a825c767fc498d43dde1b840` | MIT OR Apache-2.0 |
| CCA | github.com/storytold/craft-common-actions | `f76646c68b0910f514aea66005e09f0792865696` | **no licence file** (UNCERTAIN: treat as all rights reserved; ideas only) |
| ZZ | github.com/simpsoka/zigzag | `ad19cb09dcf5649fac55b43a083714bcd2b761d4` | Apache-2.0 (NOTICE credits Respira, Apache-2.0, for the BLE protocol, and pyembroidery, MIT) |

References below are written `KEY:path[:line]` and are relative to the repo root at the commit
in the table. PC and VC are the main sources. DC is used for its document model and data merge
(close to Labely's batch and serial printing). CL is used for its device-protocol crates
(`plotter`, `gcode`, `print`) and feature flags. CCA is used for CI shape only. Why these repos:
`gh repo list storytold` shows about 20 "craft" apps on one shared architecture. VC and PC are
the most mature, CL is where the shared hardware and printing crates live, and DC is the
nearest to labels (pages, frames, CSV merge).

---

## 1. ArtCraft Crafting Apps (PC, VC, DC, CL)

### 1.1 Workspace / crate layout
- **Shape.**
  - Libraries live in `crates/<short>` and are named `<app>-<short>`.
  - Binaries live in `apps/<app>`, `apps/<app>-cli` and `apps/<app>-web`, and there is an `xtask/`.
  - The root manifest sets `members = ["crates/*","apps/*","xtask"]` and puts shared metadata (edition 2024, MSRV 1.95, licence) under `[workspace.package]` (PC:Cargo.toml:1-10, VC:Cargo.toml:1-11).
  - All internal crates and pinned external git revisions are declared once in `[workspace.dependencies]`.
- **Numbered layers, enforced.**
  - L0 holds geometry and colour, L1 `doc`, L2 algorithms, L3 render and formats, L4 `tools`, L5 `engine`, L6 `ui-egui` and `mcp`/`automation`, L7 apps (VC:docs/architecture.md "Layers", PC:docs/architecture.md §3).
  - `cargo xtask layers` parses `cargo metadata` and fails CI on an upward or sideways dependency, or on any toolkit crate (egui, winit, rfd) below L6 (PC:xtask/src/layers.rs, VC:xtask/src/main.rs:65).
  - `testkit` may only be a dev-dependency, and that is enforced too (VC:crates/testkit/src/lib.rs header).
- **Standalone crates** (PSD, Affinity) depend on no workspace crate, so they can be published separately (PC:docs/architecture.md §3 rule 3).
- **Shared libraries** live in CL as about 120 small crates (`craft-print`, `craft-flags`, `craft-ui`, `craft-plotter`…), consumed by git `rev` pins (PC:Cargo.toml:13-60).
- **Lints, workspace-wide.**
  - `unsafe_code = "deny"`, allowed per crate with a justification (only `photocraft-tablet`).
  - Crates add `#![deny(clippy::unwrap_used, expect_used, panic, …)]` (PC:crates/ops/src/lib.rs:11-12, PC:crates/automation/src/lib.rs:18-19).

### 1.2 Command architecture
- **Declaration.**
  - Each command is a `CommandSpec`, a plain struct (VC:crates/engine/src/cmd.rs:125-141; PC:crates/engine/src/cmd.rs:15-28).
  - Fields: `id` (dotted, camelCase leaf, following the menu, e.g. `filter.blur.gaussianBlur`), `label`, `menu` path, default `shortcut`, `params` text, `enabled: fn(&Session)->Result<(),String>`, `run: fn(&mut Session,&Value)->Result<Value>` and a `journal` flag.
  - Specs are built by a small `cmd!` macro into one static table, initialised once (`OnceLock`, `HashMap` id→index) (PC:crates/engine/src/cmd.rs:51-55, 245-275).
  - No `inventory`/`linkme`: registration is explicit, which is wasm-safe.
  - Commands live under `engine/src/cmd/<area>_cmds.rs`: PC has about 700, VC about 710, DC 446.
- **UI-only commands** (windows, panels, zoom, dialogs) are a second table, `UI_COMMANDS`, in the UI crate (VC:crates/ui-egui/src/menus.rs:84; DC:docs/architecture.md "Commands").
  - MCP keeps a generated mirror so it doesn't depend on the UI crate: `cargo xtask ui-commands --check` fails CI when the mirror drifts (VC:docs/mcp.md "Command catalogue").
- **Params and schema.** The two apps have diverged here.
  - PC: `params` is a free-form "JSON-ish" hint string, such as `"width":1..300000=1920` (PC:crates/engine/src/cmd.rs:284). A separate `param_check` runs before `run` (PC:crates/engine/src/session/execute.rs:66-80).
  - The PC plan wanted `schemars::Schema` per command (PC:docs/architecture.md §4.2), but the code does not have it.
  - VC added `schema: Option<&'static ParamSchema>` from its own dependency-free crate `vectorcraft-apischema` (VC:crates/apischema/src/lib.rs). From one declaration it gives:
    - validation that names the offending parameter and fills in defaults;
    - the params text;
    - JSON Schema 2020-12 output with `x-` extensions;
    - a schema hash.
  - **API levels:** `API_VERSION` (=4) goes up whenever a built-in schema changes, and a test fails until `SCHEMA_HASHES` gains the new row (VC:crates/engine/src/schemas.rs:1-31). Params carry `added`/`deprecated` levels.
  - **Unknown keys are refused** and every batch step is checked before the first one runs (VC:crates/engine/src/schemas.rs header).
- **Dispatch.**
  - One entry point, `Session::execute(id, params) -> Result<Value, EngineError>` (PC:crates/engine/src/session/execute.rs:17; DC:docs/architecture.md).
  - The GUI's `App::run(id, params)` is "the single entry point for every frontend path" (VC:crates/ui-egui/src/app/commands.rs:74-90; PC:crates/ui-egui/src/app/commands.rs). The command palette, menus and shortcuts are built from registry metadata.
  - CLI: `<app>-cli run in.psd --cmd <id> --params '{…}' --out …`, plus `commands`, `batch`, `convert`, `info`, `mcp`, `serve` (PC:apps/photocraft-cli/src/args.rs:33-40).
  - A JSON-lines **control protocol** on loopback TCP (`{"id","method","params"}` → `{"id","ok","result"|"error"}`) exposes `engine.execute`, `engine.commands`, `ui.inspect`, `ui.pointer` and `ui.menu.invoke` (PC:docs/control-protocol.md, PC:crates/automation/src/rpc.rs:2).
  - `precheck(id, params)` validates a call without running it (PC:crates/engine/src/session/execute.rs:66).
- **Results and errors.**
  - Results are `serde_json::Value`: `null` for a plain edit, an object for anything that creates (`{"id":…}`).
  - Errors are a `thiserror` enum: `UnknownCommand` (with nearest-id hints), `Disabled(id, reason)`, `BadParams{cmd,msg}`, `NoDocument`, `UnknownLayer{id, known}`, `Cancelled` and `Other` (PC:crates/engine/src/error.rs:4-50).
  - A listing row includes `enabled`, `disabledReason` and `needsDocument`, so an agent can tell "open a document first" apart from other preconditions (PC:crates/engine/src/cmd.rs:205-224).
- **Journal.** Every journaled command is a replayable `{command, params}`, which gives recorded Actions and scripts for free (PC §4.2, DC:docs/architecture.md).
  - `Session::replay` tolerates keys that a newer schema dropped (VC:crates/ui-egui/src/app/commands.rs:93-99).

### 1.3 Document model and undo/redo
- **Model.** Plain data plus serde, with large payloads behind `Arc`, so a clone is O(nodes) pointer copies (PC:docs/architecture.md §5, PC tiles are 256² COW; DC:docs/architecture.md "Data model": items under `Arc`).
- **Snapshot undo, not inverse ops.**
  - PC's plan had `Op`/`Transaction`, but the code stores whole-document `Arc<Document>` snapshots per step, "simple, obviously correct" (PC:crates/ops/src/lib.rs:1-10, 32-60).
  - Memory is bounded by `max_states` and a byte budget that counts only unshared tiles.
  - VC stores the same, plus the selection as `Arc<Selection>` and per-step byte estimates, with a default history budget (VC:crates/engine/src/history.rs:9-40, 420).
- **One command is one undo step.**
  - Commands mutate through `Session::edit(label, |doc, active| …)`, which clones the `Arc`, runs the closure on the copy, swaps it in and pushes the old state (PC:crates/engine/src/session/execute.rs:231-280). An `Err` from the closure leaves everything untouched.
  - Queries are not journaled and push nothing.
  - PC `edit_with(record=false)` changes the document without a history step (layer visibility, by preference).
- **Coalescing.** Any command accepts `"coalesce": "<key>"`, and consecutive edits with the same key share one step, which covers slider drags and typing (PC:crates/engine/src/session/execute.rs:10-16).
  - Tools send Begin, Preview and Commit commands, so a drag is one step (DC:docs/architecture.md; VC:docs/architecture.md "Everything is a command").
- **Batches.**
  - Engine `command.batch` keeps a stash of all open documents and session state. On a failing step it rolls everything back. Otherwise each document it edited keeps the batch as one undo step (VC:crates/engine/src/cmd/fileio/batch.rs:1-60).
  - Explicit `edit.undoGroup.begin/end` commands exist as well.
- **Panic guard.** `guard::catch_panic` wraps every entry point (execute, tool events, the MCP loop, the UI frame, the control channel) and rolls back the document (VC:crates/engine/src/guard.rs).

### 1.4 MCP server design
- **PC: official SDK.**
  - `photocraft-automation` uses `rmcp` (`3.5` requested, `3.6.0` locked; features `server`, `macros`, `transport-io`) with `schemars` `1` (1.2.3 locked) and tokio (PC:crates/automation/Cargo.toml:32-35, Cargo.lock).
  - Tools are `#[tool]` methods taking `Parameters<T: Deserialize + JsonSchema>` (PC:crates/automation/src/server.rs:772-1143).
  - A `strict_schema` pass inlines `$ref`/`$defs` and replaces `true` schemas, because some providers reject them (PC:crates/automation/src/server.rs:435-490).
- **VC and DC: hand-written.** Newline-delimited JSON-RPC 2.0 over stdio, "no async runtime" (VC:crates/mcp/src/lib.rs:1-14; DC:crates/mcp/Cargo.toml). The server negotiates the protocol version from a list (VC:crates/mcp/src/server.rs:21-23, 417).
- **Transport.** Stdio only.
  - To drive a running app, the stdio server is a client of the app's loopback control port: `Backend::{Headless, Bridge, Live}` (PC:crates/automation/src/server.rs:26-35), `Remote | Headless` (VC:crates/mcp/src/backend.rs).
  - With no flag, VC tries `127.0.0.1:7979` and falls back to headless (VC:docs/mcp.md).
  - PC reads `agent-connection.json` (port plus token) before *every* call, so client configuration survives an app restart (PC:docs/mcp.md "photocraft-cli mcp").
  - Control connections use a challenge handshake: the token is never sent, and the client dials only the literal loopback address (PC:crates/automation/src/bridge.rs:1-12).
- **Granularity.** A generic core plus curated tools.
  - Generic: `command_list` / `command_run` / `command_batch` over the whole registry.
  - Curated: `doc_open/new/save/export/inspect`, `doc_render_preview`, `draw_path`, `draw_shape`, `set_paint`, `transform`, `undo`, `redo`…; about 34 tools in VC (VC:crates/mcp/src/tools/definitions.rs).
  - Bridge-only `ui_*` tools: `ui_inspect`, `ui_screenshot`, `ui_pointer`, `ui_menu_invoke`, `ui_set` (PC:crates/automation/src/server.rs:1087-1143).
  - MCP tool names use underscores, and command ids keep their dots (PC:docs/architecture.md §12).
  - Tools carry `readOnlyHint`, `destructiveHint` and `idempotentHint` annotations (VC:crates/mcp/src/tools/definitions.rs:90).
- **Batch semantics.** The two apps differ.
  - PC `command_batch`: in order, stops at the first error by default, and "not an atomic transaction", so earlier steps stay (PC:crates/automation/src/server.rs:1078-1080).
  - VC `command_batch`:
    - `atomic:true` wraps the steps in an undo group, so they form *one undo step*. Steps that ran still stay on error: grouping, not rollback.
    - Steps are capped at 256 and kept results at 8 MiB. On overflow the batch stops and says the step ran.
    - It returns `{completed, failed, results:[{ok,result|error}], created}` (VC:crates/mcp/src/tools/dispatch.rs:328-417).
- **Result references.**
  - A param string that is exactly `$N` or `$N.path` takes that value from step N's result; `$last.path` takes it from the step before. Paths walk keys and indices (`$2.ids.0`), and `$$` escapes a literal `$`.
  - A reference that resolves to nothing is an error, so a step never runs with an unresolved reference.
  - The resolver lives in the engine, so the CLI `run` and MCP share it (VC:crates/engine/src/steps.rs:1-10).
- **Headless.** The in-process `Session` renders with the CPU renderer. UI-only tools return an error that names the alternative (VC:docs/mcp.md table).
- **Preview.** `render_preview` returns MCP `image` content (`image/png`, base64), and in bridge mode a window screenshot (VC:crates/mcp/src/tools/handlers.rs:536; PC:crates/automation/src/server.rs:979-1010).
- **Sandboxing and consent.**
  - `--automation-read-root` / `--automation-write-root` capabilities (`AuthorizedWorkspace`, `cap-std`) (PC:crates/automation/src/workspace.rs; VC:docs/architecture.md "Agent control").
  - The suite-wide "let agents use Craft apps" switch is gated by the `AI-MCP` runtime flag (PC:docs/mcp.md).
- **Long work.** Long commands become jobs (`wait:false` → `{job}`), with `jobs_list` and `jobs_cancel`, and progress over MCP (PC:crates/automation/src/server.rs:1057-1076).

### 1.5 egui structure
- egui/eframe 0.37 on wgpu, the suite's own egui fork pinned by `rev` (PC:Cargo.toml:225).
- **The UI crate is a library** (`<app>-ui-egui`) and the desktop binary is thin, so tests can drive `App::frame(ui)` with plain egui.
- **App struct.** `VectorcraftApp` / `PhotocraftApp` holds the engine `Session` plus UI-only state (dock, panels, tool, dialogs, prefs).
  - Engine documents are read through snapshots and view-models (`LayersModel` rows…), never by holding `&mut` (PC:docs/architecture.md §4.1, §4.5).
- **Mutation paths.** Every mutation goes through `app.run(id, params)`.
  - It intercepts a few ids for confirmation prompts, applies the automation authorisation gate, and reports errors in the status bar (PC:crates/ui-egui/src/app/commands.rs).
  - Dialogs for filters and adjustments are generated from param schemas. Custom widgets exist only for a few complex dialogs (PC §4.5).
- **Tools** are state machines in their own L4 crate: pointer events in; commands and overlay primitives (data) out (PC §4.3; VC `crates/tools`).
- **Shared UI** (docking, tabs, i18n, IME, printing dialogs) comes from `craft-ui` (CL:crates/ui/src).

### 1.6 Feature flags
- **Cargo features.**
  - Optional native or C codecs: `heif`, `bundled-fonts`, `gpu`/`cpu-only` (PC:docs/architecture.md §3 rule 5; VC:crates/engine/Cargo.toml `[features]`).
  - Test-only generators: `testgen` (PC:crates/automation/Cargo.toml dev-deps), `corpus`.
  - CI builds without the default features too (PC:.github/workflows/ci.yml:258-302).
- **Runtime feature flags.** A separate crate, `craft-flags` (CL:crates/flags/src/lib.rs:1-30):
  - every flag is off by default;
  - one per-user `flags.toml` is shared by all apps;
  - `CRAFT_FLAGS=AI-MCP,-X` overrides for one run.
  - This lets unfinished features ship dark.

### 1.7 Hardware / device layer
- **Protocol crates do no I/O.**
  - `craft-plotter` turns a millimetre-based `Job` into a `Program` (bytes, framing, and the points where the sender must wait for the device) for HP-GL, DMPL and GP-GL/Silhouette. It also has the device queries and reply parsers (CL:crates/plotter/src/lib.rs:1-12).
  - `craft-gcode` is the same for Grbl. It offers a file writer and a "bare" streaming writer, plus real-time commands and reply parsers (CL:crates/gcode/src/lib.rs).
  - "Opening a port … polling for replies are the caller's business, and so is what a particular machine can do."
- **System printing.** `craft-print` lists printers and submits PDFs through CUPS (`lpstat`/`lp`) or the Windows spooler (CL:crates/print/src/lib.rs). `photocraft-ipp` is a dependency-free, bounded IPP client over the CUPS socket (PC:crates/ipp/src/lib.rs:1-10).
- **Tablet input.** `photocraft-tablet` is the one crate allowed `unsafe` (AppKit, Wintab, XInput).
- **Untrusted input.** File readers are "bounded, fail closed" and fuzzed (CL:crates/silhouette-studio; PC CI fuzz matrix).

### 1.8 Testing
- **Volume and placement.** About 8,000 tests in PC.
  - Unit tests sit beside the code (`*_tests.rs` sibling modules in PC, `src/tests/` folders in VC).
  - Integration tests use one test binary per crate: `tests/it/main.rs` plus modules (VC:crates/render/tests/it, DC:crates/mcp/tests/it).
- **Render checks.** VC's render "golden" tests are *pixel-sample assertions on small fixtures*, not committed PNGs.
  - `testkit::raster` offers `render_view`, `assert_pixel`/`assert_rgb` with a tolerance, `diff`, `perceptual_diff` and `assert_similar(tol, max_bad_fraction)` (VC:crates/render/tests/it/golden.rs:1-15, VC:crates/testkit/src/raster.rs:55-165).
- **Oracles.**
  - PSD files carry Photoshop's own composite, which serves as a free conformance oracle.
  - CPU/GPU parity is checked to ≤1/255 on lavapipe in CI (PC §7.2, §7.4).
  - External oracles (ffmpeg, Ghostscript) are found at run time and print SKIPPED unless `…_REQUIRE_ORACLES=1` is set (CL:crates/media-testkit, CL:crates/plate-test).
- **UI.**
  - `egui_kittest` (wgpu, snapshot) harnesses for panels.
  - `craft_ui::testing::Screens` renders a screen once per test language (Arabic, Japanese…) with no system fonts, so snapshots are machine-independent, and compares `tests/snapshots/<name>-<lang>.png`. `UPDATE_SNAPSHOTS=1` rewrites them (CL:crates/ui/src/testing/mod.rs:1-30).
- **Engine and fuzz.**
  - Command-level tests open a fixture, dispatch commands and assert on `inspect` or a render.
  - `proptest` strategies generate engine command sequences and junk params for fuzzing the registry (VC:crates/testkit/src/strategies.rs).
  - Invariant and round-trip checks (VC:crates/testkit/src/invariants.rs).
- **Corpora.** Fetched by `cargo xtask corpus` at pinned commits with SHA-256 checks, never committed (VC:docs/architecture.md "Corpus fetching").
- **Runner.** `cargo nextest` with a pinned minimum version (PC:.config/nextest.toml).

### 1.9 Packaging / CI / releases
- **Gate.** CI calls one gate, `cargo xtask ci`: fmt, clippy `-D warnings`, `layers`, `ui-commands --check`, `i18n-report`, a wasm build and tests. It runs through a reusable workflow, `craft-common-actions/.github/workflows/gate.yml@main` (PC:.github/workflows/ci.yml:1-5, 129; VC:xtask/src/main.rs:300-320).
- **Other CI.** Separate jobs for Windows, without-feature builds, corpus, a web size gate and a fuzz matrix. Workflows also cover house rules, packaging lint, SonarCloud, nix and FreeBSD.
- **Releases.** A shared `release.yml` builds and signs every platform (CCA:README.md):
  - macOS universal DMG, Developer ID signed and notarised;
  - Windows MSI (WiX), Azure Trusted Signing;
  - Linux AppImage, deb, rpm (nfpm), Flatpak, Arch, FreeBSD and wasm;
  - a signed `SHA256SUMS`, published to a draft GitHub release.
- **Cutting a release.** `scripts/cut_release.py` bumps the version on the newest green commit of `main` and merges it into the `release` branch. Nightly dry runs build everything unsigned (PC:.github/workflows/release.yml:4).
- **Self-update.** `craft-update` handles release checks and installs (PC:Cargo.toml:73).

---

## 2. ZigZag (ZZ)

### 2.1 Workspace / crate layout
- **One package.** A single crate, `zigzag` (edition 2024, MSRV 1.95), with one library and three binaries: `zigzag` (CLI), `zigzag-mcp` (stdio↔HTTP bridge) and `zigzag-studio` (requires `gui`) (ZZ:Cargo.toml:1-30).
- **Modules.**
  - `document/`: the SVG-backed model.
  - `stitch/`, `design.rs` (planner) and `route.rs`.
  - `formats/`: machine-file writers and readers, plus preview PNG.
  - `machine/`: device profiles and the BLE protocol.
  - `studio/`: about 50 modules (egui app, MCP, tools, agent panel) (ZZ:README.md:80-98).
- **Vendored code.** Two crates vendored from VectorCraft (`vectorcraft-geom`, `vectorcraft-trace`, MIT/Apache) under `vendor/`, with `opt-level=3` in dev for speed (ZZ:Cargo.toml:58-82).

### 2.2 Command architecture
- **Three surfaces, not one registry.**
  - **GUI:** `enum Action` (New, Open, Undo, FlipH, `Kind(Kind)`…) plus `menus()`, which drives the macOS menu bar, the in-window menus and shortcuts from one list (ZZ:src/studio/commands.rs:1-180, 480-520). Actions call `Studio` methods directly.
  - **MCP:** about 35 hand-written tools (`get_design`, `add_svg`, `set_object`, `transform_objects`, `add_text`, `render_preview`, `check_design`, `export`, `send_to_skitch`, `machine_status`, `batch`…).
    - Schemas are hand-written `serde_json::json!` blocks in `tool_specs()` (ZZ:src/studio/tools.rs:177-540).
    - Dispatch is one `match name` in `Studio::run_tool(name, &Value) -> ToolResult` (ZZ:src/studio/tools.rs:977-1275).
  - **CLI:** clap subcommands (`convert`, `info`, `fabrics`, `machines`, `skitch …`, `capture`) over the library, not over tools (ZZ:src/main.rs; README "CLI").
  - Shared mutation primitives (`Studio::structural`, `Document` edit methods) keep the three consistent. Nothing makes them so.
- **Agent ergonomics worth copying.**
  - **Unknown arguments are refused.** The error lists the accepted keys (a misspelt `widht_mm` is not silently ignored) (ZZ:src/studio/tools.rs:601-643).
  - **Units in parameter names** (`x_mm`, `size_mm`, `width_px`), and a coordinate convention stated in the server `instructions` (ZZ:src/studio/tools.rs:15-27).
  - **Objects can be named three ways:** by index, display name or SVG id, with the resolver shared by all tools (ZZ:src/studio/tools.rs:700).
  - **`terse: true`** on any edit (or on a whole batch) returns `{ok, added:[names], new_problems:[…]}` instead of the full object. Problems are diffed by a measurement-free key, so "a bit smaller" doesn't count as new (ZZ:src/studio/tools.rs:560-575, 1291-1326).
  - **Read-only tools** are listed once (`only_reads`) and turned into `readOnlyHint`/`destructiveHint` annotations (ZZ:src/studio/tools.rs:520-556).
- **Results.**
  - `ToolResult{content: Vec<Content::{Text,Image(png)}>, is_error}`. JSON is pretty-printed into a text item (ZZ:src/studio/mcp.rs:20-64).
  - Errors are plain sentences that say what to do next ("call list_fonts").

### 2.3 Document model and undo/redo
- **The model is the SVG text.**
  - Every drawable element gets a stable id.
  - Stitch settings, threads and order are written back *into the ids*; document settings go into a `data-zigzag` attribute.
  - Layout edits become SVG `transform`s. Everything else is preserved byte for byte, so Illustrator round-trips (ZZ:src/document/mod.rs:1-8, 253-266; ZZ:src/document/edit.rs:1-4).
- **Snapshot undo.**
  - Each step is a whole `Document` clone (about 0.5 MB for a traced logo), capped at `UNDO_STEPS = 200` (ZZ:src/studio/app.rs:89-90, 687-697).
  - `structural(f)` clones, applies `f`, then either pushes the clone or restores it on `Err` (ZZ:src/studio/app.rs:699-720).
- **GUI edits are detected, not dispatched.**
  - Widgets mutate `doc` in place. At the end of each frame a cheap fingerprint (`Snap`) is compared with the last stable state.
  - The first change in a burst pushes one undo step, so a slider drag coalesces. Drags and dialog previews are excluded (ZZ:src/studio/app.rs:1176, 1272-1290).
- **MCP edits.** Each MCP call is one undo step (ZZ:README.md "Agents (MCP)"). Each tab keeps its own history.
- **Recovery.** Session and per-tab recovery files are written whole-or-nothing (ZZ:README.md "Picks up where you left off").

### 2.4 MCP server design
- **Inside the studio, over Streamable HTTP.**
  - `tiny_http` 0.12 on `127.0.0.1:7878/mcp`, one thread per request so a slow call can't block pings. JSON responses only: no SSE, and `GET` returns 405 (ZZ:src/studio/mcp.rs:1-8, 150-320).
  - **Hand-written JSON-RPC.** No MCP crate. It answers `initialize` (negotiating 2025-06-18, 2025-03-26 or 2024-11-05), `ping`, `tools/list`, `tools/call` and empty resources and prompts, plus JSON-RPC batch arrays (ZZ:src/studio/mcp.rs:17, 329-385).
  - **Browser defence:**
    - an `Origin` that is not localhost gets 403, and `null` is refused too;
    - `POST` must be `application/json`, so a page can't avoid a CORS preflight;
    - the body is capped at 32 MiB (ZZ:src/studio/mcp.rs:206-260).
- **UI-thread execution.**
  - The HTTP thread sends a `Call{name, args, reply: Sender, deadline}` over an `mpsc` channel and wakes egui.
  - `Studio::handle_mcp` drains calls between frames. It *defers* them while a dialog preview or a drag is active and drops calls whose 120 s deadline has passed, because the client was already told they failed (ZZ:src/studio/mcp.rs:66-77; ZZ:src/studio/app.rs:1101-1131).
  - The status bar shows an MCP chip (waiting, connected, busy or failed; clients; recent calls) (ZZ:src/studio/mcp_chip.rs).
  - The agent's working objects get an animated outline on the canvas (ZZ:src/studio/agent.rs:761-890).
- **Stdio and headless.**
  - `zigzag-mcp` is a stdio↔HTTP bridge to the running studio. With no studio running it returns a JSON-RPC error and keeps running (ZZ:src/mcp_bridge.rs:1-40).
  - `--headless` starts `zigzag-studio --mcp-stdio`: the same `run_tool` on a window-less `Studio`, with an extra sentence in the instructions ("no one is watching") (ZZ:src/studio/main.rs:100-117; ZZ:src/studio/mcp.rs:387-417).
- **Granularity.** Curated domain tools only. No generic "run any command" tool.
- **Atomic batch.** `batch{steps:[{tool, arguments}], atomic=true, terse, label}` (ZZ:src/studio/tools.rs:1328-1470).
  - Up to 64 steps. Nested batches are refused, and so are steps that can't be rolled back (`undo`, `open_file`, `switch_document`, `new_document`).
  - Before the first step the batch saves the doc, the undo/redo stacks, the selection and the modified flag.
  - On failure it restores all of them and says "Nothing was changed". On success it truncates history to *one* undo step.
  - Expensive replanning is deferred to the end of the batch, except before steps that read stitches.
  - The step shape is validated: `args` instead of `arguments` is an error, not a silent no-op.
- **Result references.** `"$N.path"` with numeric segments indexing arrays (`$0.added.0.name`), and `"$$"` for a literal `$`. A bad step number or a missing field is an error (ZZ:src/studio/tools.rs:644-680).
- **Preview.** `render_preview{width_px 200–2000, show_jumps, fit: design|hoop}` returns a PNG `image` content item (ZZ:src/studio/tools.rs:2194-2247).

### 2.5 egui structure
- **Stack.** `eframe` 0.37 on **glow** (OpenGL) only, without wgpu, for a smaller and simpler build. `muda` provides the native macOS menu (ZZ:Cargo.toml:44-50, 62-66).
- **App state.** `Studio` is one large struct: `doc`, `stable`, the planned `design`, `issues`, `names`, `undo`/`redo`, `batching`, the selection, the dock, the MCP server and tabs (ZZ:src/studio/app.rs:92+).
  - A testable `frame(&mut self, ui)` is kept apart from `eframe::App` (ZZ:src/studio/app.rs:1133).
- **Layout.** A hand-made Illustrator-style dock (`dock.rs`, about 3.7 kLOC), tool flyouts and a contextual task bar.
- **Background planning.** Work over 12 ms moves to a worker (`planner.rs`).
- **Routing.** The GUI does *not* route through MCP tools. MCP tools reuse the same `Studio` methods.

### 2.6 Feature flags
- `default = []` keeps the core pipeline (CLI plus bridge) free of system dependencies.
- `gui` adds eframe, rfd and `tiny_http`. `ble` adds btleplug 0.13 (0.13.5 locked), tokio and uuid, with libdbus vendored on Linux.
- `menu-check` compiles the native menu code on Linux (ZZ:Cargo.toml:67-90).
- Device modules are `#[cfg(feature = "ble")]` only at the transport (ZZ:src/machine/mod.rs:6).

### 2.7 Hardware / BLE layer
- **Protocol is pure.** `machine/protocol.rs` is "transport-free so it can be unit tested":
  - a big-endian u16 command id plus payload;
  - replies of the form `[cmd_hi, cmd_lo, status, …]`;
  - status enum and error-help table;
  - typed parsers for info, progress and pattern info;
  - 500-byte upload chunks;
  - a 24-byte layout payload (ZZ:src/machine/protocol.rs:1-10, 338-540).
- **`Transport` trait.** `request(cmd, payload)->reply`, `send` (no reply), `read` (re-read a late reply), `connected`, `close` and `settle()` (a delay after streamed chunks) (ZZ:src/machine/session.rs:12-31).
  - `Session::open(Box<dyn Transport>)` provides blocking high-level calls (`info`, `state`, `upload`, `start_sewing`…) and runs on a worker thread (ZZ:src/machine/session.rs:55-215).
- **Implementations.**
  - `skitch.rs`: btleplug, behind `ble`. It documents the pairing quirk: an unpaired machine connects but replies empty, which surfaces as a `NotPaired` error.
  - `sim.rs`: a **simulated machine** that steps through the real state machine (homing, button presses, thread changes), so the whole flow is tested without hardware (ZZ:src/machine/sim.rs).
- **Reverse-engineering tools.**
  - `capture.rs` parses Apple PacketLogger `.pklg` and Android `btsnoop_hci.log`, reassembles ATT writes and notifications, and lists the conversation by command name.
  - A "settings bytes" diff view supports a change-one-setting workflow.
  - Writing unknown settings is withheld until a round-trip proves it safe (ZZ:src/machine/capture.rs:1-13; README "Working out undocumented commands").
- **Device profiles are data.** Built-in `profiles.json` has the same shape as user profiles, and applying a profile copies its settings into the document, so a design opens anywhere (ZZ:src/machine/profile.rs:1-10).

### 2.8 Testing
- **Coverage.** 136 library tests and 153 editor/MCP tests, with and without `ble` (ZZ:README.md "Status").
  - Tests are inline `#[cfg(test)]` modules, and the tool tests sit at the bottom of `tools.rs` (ZZ:src/studio/tools.rs:2348-3650).
- **MCP tests.**
  - a full agent workflow;
  - a real HTTP JSON-RPC session on a port, including the Origin and Content-Type refusals;
  - "atomic batch is one undo step and passes results on";
  - "failing atomic batch changes nothing";
  - misspelt argument refusal;
  - "tools say which only read";
  - a well-formed tool list.
- **UI.** `egui_kittest` harnesses for UI interactions (ZZ:src/studio/dock.rs:3265).
- **Oracles.** Every machine-file writer is read back through pyembroidery with identical stitch positions, and PEN output is checked against hand-computed sequences. There are no golden images.

### 2.9 Packaging / CI / releases
- **macOS.** Local scripts: `packaging/macos/make-app.sh --release` builds a universal app, Developer-ID signs it, notarises it with `notarytool` and makes a DMG. `publish-release.sh` and a Homebrew cask updater go with it.
- **Linux and Windows.** GitHub workflows triggered on `release: published` or `workflow_dispatch(tag)`.
  - They check that the Cargo version matches the tag, build `--locked --release --features gui,ble` and attach a zip or tarball (ZZ:.github/workflows/linux-release.yml:3-160, windows-release.yml).
  - There is no PR test workflow at this commit.
- **Licences of dependencies.** Generated by `packaging/licenses/generate.sh` (cargo-about style `about.toml`).
- **In-app updater.** It checks GitHub releases, verifies the checksum and code signature, and relaunches (ZZ:src/studio/update.rs).

---

## 3. Differences

| Topic | Crafting Apps | ZigZag | Labely takes |
|---|---|---|---|
| Single command surface | One `CommandSpec` registry; GUI, CLI, control channel and MCP all call `execute(id, params)` | Three surfaces (`Action`, MCP tools, clap) over shared methods | Crafting Apps |
| Param schema | PC: hint strings; VC: own `apischema` crate with API levels and a hash | Hand-written JSON per tool, unknown keys refused | Typed structs + `schemars`, plus VC's API level and hash |
| Undo | `Arc` structural-sharing snapshots; `coalesce` key; batch is one step per document | Whole-clone snapshots; GUI edits found by diffing a fingerprint each frame | Arc snapshots, explicit per command |
| Batch atomicity | PC: none; VC MCP: one undo group, no rollback; VC engine: full rollback | All or nothing by default, with rollback | Atomic with rollback by default |
| Result references | `$N.path`, `$last`, `$$`; resolver in the engine | `$N.path`, `$$`; resolver in the MCP layer | `$N.path`, `$last`, `$$`, in core |
| MCP crate | PC: `rmcp` 3.x + schemars; VC/DC: hand-written, no async | Hand-written, `tiny_http` | `rmcp` (§4.6) |
| MCP transport | Stdio only; stdio bridges to the app's token-auth TCP control port | Streamable HTTP in the studio; stdio bridge binary; `--mcp-stdio` headless | Both: HTTP in the studio, stdio in the CLI |
| Tool granularity | Generic `command_*` + curated + `ui_*` | Curated only, `terse` option | Generic + a curated few + `terse` |
| UI thread | Control requests yield after a bounded batch per frame | Channel to the UI thread, deadline, deferred during drag or dialog | ZigZag |
| Renderer | wgpu (GPU composite), CPU reference | glow only | Labely-specific (§4.9) |
| Device layer | Protocol crates with no I/O; system print spooler | Pure protocol + `Transport` trait + simulator + capture parser | ZigZag shape, split into crates as CL does |
| Golden images | Pixel samples and perceptual diff; kittest PNG snapshots with `UPDATE_SNAPSHOTS` | None (format oracles) | Exact 1-bit goldens + kittest snapshots |
| Runtime flags | `craft-flags` (user TOML + env) | Cargo features only | Cargo features now; runtime flags later |
| CI | `cargo xtask ci` gate + reusable workflows + fuzz + layers | Release-only workflows; local Mac signing | Crafting Apps, scaled down |

---

## 4. Conventions Labely adopts

### 4.1 Workspace and crates
1. **Layout.**
   - Libraries go in `crates/<dir>`, with packages named `labely-<dir>`: `core`, `render`, `protocol`, `transport`, `printers`, `mcp`, `studio` (egui as a library), `ffi` (UniFFI) and `testkit` (dev-only).
   - Thin binaries go in `apps/labely-cli` (binary `labely`) and `apps/labely-studio`, plus `xtask/`.
   - Android and iOS shells go in `mobile/android` and `mobile/ios`.
   - This follows PC/VC: the UI as a library makes kittest possible, and thin binaries keep wiring in one place.
2. **Workspace manifest.** `[workspace.package]` holds edition 2024, `license = "MIT OR Apache-2.0"` and MSRV. Every dependency is declared once in `[workspace.dependencies]`, and `Cargo.lock` is committed.
3. **Layers.**
   - L0 `protocol`; L1 `core`; L2 `render`, `printers`; L3 `transport`; L4 `mcp`, `ffi`, `studio`; L5 apps.
   - `cargo xtask layers` fails CI on an upward edge, or on `egui`/`eframe`/`winit`/`rfd`/`btleplug` appearing in `core`, `render` or `protocol`.
4. **Lints.** `unsafe_code = "deny"` workspace-wide, allowed only in `ffi` and `transport`, each with a comment. Library crates deny `unwrap`, `expect`, `panic`, `todo` and `unreachable` outside tests.

### 4.2 Commands
5. **Ids.**
   - Dotted lowercase `area.verb[_object]` with snake_case segments: `doc.new`, `element.add_text`, `element.set_props`, `print.start`, `device.connect`.
   - JSON params are snake_case, with units in the name (`x_mm`, `width_mm`, `rotation_deg`, `density`), as ZZ does.
   - MCP-facing names swap dots for underscores only where a tool *is* a command alias.
6. **Command trait.** Shape, in `labely-core::command`:
   - `trait Command { const ID: &'static str; type Params: DeserializeOwned + JsonSchema; type Output: Serialize + JsonSchema; fn meta() -> CommandMeta; fn enabled(&Ctx) -> Result<(), Disabled>; fn run(&mut Ctx, Self::Params) -> Result<Self::Output, CommandError>; }`.
   - `CommandMeta` holds `label`, `menu: &[&str]`, `shortcut`, `kind: Query | Edit | Device | Ui`, `undoable` and `journal`.
   - Params structs use `#[serde(deny_unknown_fields)]`.
   - Registration is an explicit list in `core/src/registry.rs` (no `inventory`, safe for FFI and static linking), type-erased to `fn(&mut Ctx, Value) -> Result<Value, CommandError>`.
   - This takes PC/VC's single registry but with typed params, because hint strings (PC) drift and a home-grown schema crate (VC) is avoidable at Labely's size.
7. **Schema crate.** `schemars` 1.x derives the schemas. `labely-core` has three jobs:
   - output 2020-12 schemas with `$ref` inlined for strict MCP clients (PC `strict_schema`);
   - expose `API_VERSION: u32`;
   - keep a schema-hash table, with a test that fails until a changed schema bumps the API version (from VC).
8. **One entry point.** `Session::execute(id: &str, params: Value) -> Envelope` is the only way to change a document.
   - Studio menus, palette and shortcuts are generated from `CommandMeta`. Studio-only actions (panels, zoom) are `kind: Ui` commands in a `UI_COMMANDS` table in `studio`, and `xtask ui-commands --check` keeps MCP's mirror in sync.
   - The CLI has `labely run <file> --cmd <id> --params '<json>'`, `labely run --script steps.json`, `labely commands [--json]` and `labely mcp`.
   - FFI exports `execute(id, params_json) -> envelope_json`, `commands_json()` and `render_png(...)`. Mobile shells hold no editing logic.
9. **Result and error envelope.** The same JSON shape on every surface:
   - success: `{"ok":true,"result":<Output>,"changed":[ids],"created":[ids],"warnings":[…]}`;
   - failure: `{"ok":false,"error":{"code":"bad_params","message":"…","param":"width_mm","accepted":["…"],"hint":"…"}}`.
   - `code` is a stable snake_case enum: `unknown_command` (with nearest ids), `bad_params`, `disabled`, `no_document`, `not_found` (with known ids), `device_unavailable`, `device_error`, `cancelled`, `internal`.
   - MCP maps `ok:false` to `isError:true` with the same JSON as text. Messages say what to do next (ZZ style).
   - `terse:true` drops `result` and keeps `ok`, `created`, `changed` and new `warnings` (ZZ).
10. **Journal.** Every `journal` command is recorded as `{id, params}`. A script is a `Vec` of those and replays through `execute`, which gives macros and CLI scripts for free (PC/VC).

### 4.3 Document model and undo
11. **Model.** Plain serde structs with stable element ids (`ElementId(u64)`, never indices). Children sit behind `Arc`, so a document clone is cheap.
    - The `.labely` native format is versioned JSON with a `format_version` field and migrations.
    - Units are mm in the model, converted to printer dots only in `render`.
12. **Undo by snapshot.**
    - History stores `Arc<Document>` plus selection per step. Each `Edit` command is exactly one undo step; `Query` and `Device` commands push nothing.
    - It is capped by step count (200) and a byte budget.
    - An `Err` inside the edit closure leaves document and history untouched.
    - PC, VC, DC and ZZ all chose snapshots. Labels are tiny, so inverse operations would add bugs and gain nothing.
13. **Coalescing.** An optional `coalesce: "<key>"` param merges consecutive same-key edits into one step (PC), for slider drags and typing.
    - Canvas gestures send `*.begin` / `*.preview` / `*.commit` commands, so a drag is one step (DC/VC).
    - No per-frame fingerprint diffing (ZZ), because that path bypasses the command layer.
14. **Panic guard.** `execute` runs under `catch_unwind` and restores the pre-call snapshot on a panic, returning `internal` (VC `guard`).

### 4.4 Batches
15. **`command.batch` lives in core** as a normal command, so the CLI, MCP and FFI share it: `{steps:[{id, params}], atomic?=true, stop_on_error?=true, label?, terse?}`.
    - **Atomic (the default)** checks every step's params before running any. It then saves the session state (documents, history, selection, active document) and runs the steps in order. Any failure restores the saved state and reports `"step N (<id>) failed: …; nothing was changed"`. On success the batch is collapsed to one undo step named `label`.
    - `atomic:false` keeps the successful steps, each as its own undo step.
    - Commands that can't be rolled back can't run inside an atomic batch: `print.*`, `device.*`, `doc.open`, `doc.close`, `history.*` and a nested `command.batch`.
    - We follow ZZ and VC-engine rather than PC/VC-MCP, because agents expect "all or nothing".
16. **Result references.**
    - A param string that is exactly `$N` or `$N.<path>` (0-based step) takes that value from step N's result; `$last.<path>` takes it from the previous step.
    - Path segments are keys or numeric indices (`$0.id`, `$1.created.0`), and a leading `$$` is a literal `$`.
    - An unresolved reference fails the batch before that step runs.
    - The resolver is `labely-core::steps`, shared with CLI scripts.
17. **Limits.** At most 128 steps, and kept results capped at 4 MiB. Reply: `{completed, failed, results:[{ok, result|error}], created:[ids]}`.

### 4.5 Studio (egui)
18. **App state.** `LabelyApp { session: Session, ui: UiState }`.
    - `UiState` holds only view state: dock, tool, zoom, open dialogs, hover.
    - Panels read view-models built from `session` and never mutate documents.
    - Every mutation goes through `app.run(id, params)`, which shows errors in the status bar. `frame(&mut self, ui)` is kept apart from `eframe::App` for kittest.
19. **MCP calls on the UI thread.**
    - Calls reach the UI thread through an `mpsc` channel with a reply sender and a deadline, and the HTTP side wakes egui with `request_repaint`.
    - Calls are drained between frames, deferred while a gesture or a dialog preview is open, and dropped after their deadline (default 60 s).
    - A status-bar MCP indicator shows connection, recent calls and failures, and the canvas highlights elements the agent touched (ZZ).
20. **Renderer.** eframe with `glow` by default and `wgpu` as an optional feature. Labely's canvas is a 1-bit raster plus vector overlays, so a GPU compositor isn't needed. This keeps the build small (ZZ).

### 4.6 MCP
21. **Crate.** `rmcp` (the official SDK, the 3.x line PC pins), with `schemars` tool params, in `labely-mcp`.
    - It covers stdio and Streamable HTTP and tracks protocol revisions upstream. Labely already needs tokio for btleplug, so the "no async runtime" reason ZZ and VC give for writing their own doesn't hold.
    - UNCERTAIN: confirm the feature name of rmcp 3.x's Streamable-HTTP server transport, and that it can return plain JSON instead of SSE, before relying on it. If it can't, use ZZ's single-endpoint JSON approach behind the same `ServerHandler`.
22. **Transports.**
    - **In the studio:** Streamable HTTP on `127.0.0.1:<port>/mcp` (default 7878, configurable, `--no-mcp` to turn off). It refuses a non-local `Origin` (including `null`) and any non-JSON `POST` (ZZ). It publishes `{port, token}` to `<app-data>/mcp-connection.json` (0600).
    - **`labely mcp` (stdio):** reads the connection file before every call and forwards to the running studio. When none is running, it serves an in-process headless `Session` (PC `Live`, VC fallback). `--headless` forces headless; `--connect` forces bridge.
    - Logs go to stderr only.
23. **Tools.**
    - Generic: `command_list` (filter, schemas, enabled/disabled reason), `command_run{id, params}` and `command_batch` (§4.4).
    - Curated: `document_get`, `document_new`, `document_open`, `document_save`, `render_preview`, `device_list`, `device_status`, `print`, `undo` and `redo`.
    - All carry `readOnlyHint`/`destructiveHint` from `CommandMeta.kind`, and server `instructions` state units and coordinates.
    - This mixes VC's completeness with ZZ's ergonomics. Generating one tool per command would mean too many tools for clients.
24. **Preview.** `render_preview{width_px?, scale?, show_margins?}` returns an `image/png` content item of the *print raster* (what the printer will get), plus a text item with `{width_dots, height_dots, dpi}`. In bridge mode the same render comes from the studio, not a window screenshot.
25. **Safety.**
    - Agent file access is limited to `--read-root` / `--write-root` (PC `AuthorizedWorkspace`).
    - `print` from an agent requires the studio's "Allow agents to print" setting, which is off by default. Headless agents may print only with `--allow-print`.

### 4.7 Hardware
26. **`labely-protocol` does no I/O** (CL plotter, ZZ `protocol.rs`). It contains:
    - packet framing and checksums;
    - typed request builders and reply parsers;
    - status and error enums with user-facing help;
    - raster line encoding;
    - a `Program` that marks the points where the sender must wait.
    - It is fuzzed, and readers are bounded and fail closed.
27. **`labely-transport`.**
    - `trait Transport: Send { fn request(&mut self, frame: &[u8]) -> Result<Vec<u8>>; fn send(&mut self, frame: &[u8]) -> Result<()>; fn read(&mut self) -> Result<Vec<u8>>; fn connected(&self) -> bool; fn close(&mut self); fn settle(&self) -> Duration; }`.
    - Implementations: `ble` (btleplug), `usb`/`serial` (feature-gated) and `sim`, a stateful printer simulator that is always compiled in for tests and demos.
    - A blocking `PrinterSession` runs on a worker thread, and the UI and MCP talk to it through `device.*` commands.
28. **`labely-printers`.**
    - Printer models and paper live in data files (`printers/*.json`): dpi, print width, protocol variant and quirks. Built-in and user profiles share one schema (ZZ `profiles.json`).
    - A document stores the profile settings it needs, not just the id.
29. **Capture tooling.** `labely capture <btsnoop|pklg>` decodes recorded vendor-app traffic into named commands, to support clean-room protocol work (ZZ). Unknown settings writes stay disabled until read-back verification passes.

### 4.8 Feature flags
30. **Cargo features.**
    - The core default builds with no system dependencies.
    - `studio` features: `gui`, `wgpu`. `transport` features: `ble`, `usb`, `serial`.
    - `apps/labely-cli` enables `ble` and `usb` by default. `ffi` enables `ble` but not `gui`.
    - CI builds `--no-default-features` and `--all-features`.
    - Runtime flags (a `labely-flags` crate on CL `craft-flags`'s pattern: off by default, a TOML file plus `LABELY_FLAGS` env) are added when the first unfinished feature needs to ship dark.

### 4.9 Testing
31. **Layout.**
    - Unit tests go in sibling `*_tests.rs` or `tests.rs` modules.
    - Each crate has one integration binary, `crates/<c>/tests/it/main.rs`.
    - `labely-testkit` (dev-only, enforced by `layers`) holds fixtures, a `DocBuilder`, proptest strategies for command sequences and junk params, and invariant checks (VC testkit).
32. **Golden images.**
    - Render output is 1-bit and deterministic, so goldens are **exact**: `testkit::assert_golden(name, &raster)` compares against `crates/render/tests/golden/<name>.png` and writes `<name>.actual.png` and `<name>.diff.png` on mismatch.
    - `LABELY_BLESS=1` (or `cargo xtask bless`) rewrites them, and reviewers see the PNG diff in the PR.
    - Printer byte streams have their own goldens (`tests/golden/<model>/<name>.bin`), compared as hex dumps.
    - This differs from VC's sampled pixels because exact 1-bit output allows strict comparison.
33. **UI snapshots.** `egui_kittest` with the `snapshot` feature (`UPDATE_SNAPSHOTS=1`), with bundled fonts only so results are machine-independent (CL `craft_ui::testing`).
34. **Required test classes.**
    - Every command has a test that dispatches it and asserts on the document or envelope.
    - Batch tests: one undo step; a failing batch changes nothing; references resolve.
    - Unknown keys are refused, with the accepted keys listed.
    - Schema hash and API version.
    - An end-to-end MCP session over HTTP (Origin and Content-Type refusals included) and over stdio headless.
    - A full print flow against the simulator, with and without `ble` (ZZ).

### 4.10 CI and releases
35. **`cargo xtask ci`** is the single gate, and CI just calls it:
    - `fmt --check`, `clippy --all-targets -D warnings`, `layers`, `ui-commands --check` and schema-hash check;
    - `cargo nextest run --locked` (with a pinned nextest), doctests and `cargo deny check`;
    - builds `--no-default-features` and `--all-features`.
    - Matrix: Linux (gate), macOS and Windows (tests).
    - Mobile jobs: `cargo ndk` for `aarch64-linux-android` and an `xcframework` build for `ffi`, with a UniFFI bindings drift check.
    - Fuzz targets for `protocol` parsers and `.labely` load run nightly.
36. **Releases.**
    - A tag `vX.Y.Z` triggers `release.yml`, which checks that the Cargo version matches the tag (ZZ) and builds:
      - a macOS universal DMG, Developer-ID signed and notarised;
      - a Windows zip/MSI;
      - a Linux tarball and AppImage;
      - CLI archives;
      - `SHA256SUMS`;
      - a draft GitHub release.
    - Signing secrets live only in CI.
    - Third-party licence notices are generated at release (`cargo about`).
    - CCA's reusable workflows are a model only: that repo has no licence, so we write our own.
