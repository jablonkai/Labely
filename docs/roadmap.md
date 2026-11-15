# Roadmap

Each phase ends with a green build (fmt, clippy `-D warnings`, tests), a short summary, and a stop
for approval. Plan tags in [`feature-matrix.md`](feature-matrix.md) (Ph1–Ph6) refer to these phases.

## Phase 0: Research (this phase)

- **Deliverables:**
  - `docs/prior-art.md`
  - `docs/feature-matrix.md`
  - `docs/protocol.md`
  - `docs/printers.md`
  - `docs/architecture.md`
  - this roadmap
  - ADRs 0001–0011
- **No product code.**

## Phase 1: Engine + CLI, real print on B21 Pro

1. **Workspace scaffold:** workspace, `rust-toolchain.toml`, licences (MIT + Apache-2.0 files),
   `xtask` (ci, layers, bless), GitHub Actions CI on macOS, Linux and Windows.
2. **`labely-protocol`:** NIIMBOT framing, command builders and parsers, row encoding (0x84 blank,
   0x85 bitmap; no 0x83), status and RFID parsing.
   - Byte-golden tests.
   - Fuzz target.
3. **Fixture capture:** the user records a PacketLogger (.pklg) trace of the vendor app printing to
   the B21 Pro. We observe behaviour only.
   - `labely capture` normalises the trace into `fixtures/niimbot/b21_pro/*.json`.
   - Decoder tests run against those fixtures.
   - The open questions at the end of protocol.md and printers.md are resolved.
4. **`labely-core`:**
   - `Document`, the `.labely` v1 format, the Command trait and registry, `Session::execute`, the
     envelope.
   - Snapshot undo, `command.batch` with references.
   - About 15 commands: doc, text, barcode, QR, image, shape, line, set_props, move, delete, undo,
     redo.
5. **`labely-render`:** tiny-skia + cosmic-text + rxing; threshold and Floyd–Steinberg dithering;
   golden 1-bit PNG tests at 203 and 300 dpi.
6. **`labely-transport`:** btleplug BLE, serialport USB CDC, the sim transport.
7. **`labely-printers`:** Driver trait, JSON model registry (all NIIMBOT models from printers.md),
   `NiimbotDriver`, `FakePrinter`, single-printer `DeviceHub`.
8. **`labely-cli`:** `run`, `commands`, `render`, `check` (basic), `print`, `devices`.
9. **Exit criteria:**
   - `labely print sample.labely --printer ble:<name>` and `--printer usb:…` both print a correct
     label on your B21 Pro: correct size, density and orientation, with text, Code128 and QR.
   - Printing the same file to FakePrinter matches the golden PNG.

## Phase 2: Command registry + MCP + studio MVP

- **Full command surface:** schemas, `API_VERSION` and schema-hash test, journal and scripts,
  `doc.check` with all rules in architecture §3.4.
- **`labely-mcp`:** stdio headless and bridge modes; Streamable HTTP inside the studio;
  `render_preview`, `check`, print gating.
- **`labely-studio` MVP:** open, edit and save; canvas with select, move, resize and rotate;
  properties panel; layers; undo; preview; print dialog; printer panel. Everything goes through
  `app.run`.
- **Docs and i18n:** AGENTS.md and `docs/agents/` (usage, tool catalogue, batch examples); EN/HU
  Fluent setup.
- **Exit criteria:**
  - An agent builds a label through MCP (batch with `$0.id` references), previews it, checks it and
    prints it to FakePrinter.
  - The same works in the open studio, live.

## Phase 3: Editor, templates and batch printing to parity

- **Elements:** all element types and properties from the feature matrix (tables, date/time,
  serial, icons, borders, vertical and curved text, rich text).
- **Editor:** align, distribute, group, lock, snapping, grids, shortcuts.
- **Data:** CSV and XLSX sources (calamine) with field binding, variables, serial numbers, record
  ranges.
- **Printing:** print queue. Printing to several printers at once moved to later (ADR-0019).
- **Templates:** an original starter set, local library and folders, import and export (PNG, PDF,
  `.labely`, image import, multi-image batch).
- **Device:** label paper database (local), RFID paper auto-match, calibration, and device settings
  where the protocol supports them.
- **Exit criteria:** every Ph1–Ph3 row in the feature matrix is done or has a stated reason.

### Phase 3 progress

Phase 3 lands in slices, one pull request each.

| Slice | Scope | Status |
|---|---|---|
| 3.1 Data | CSV/XLSX sources, column binding, serial counters, record ranges, copies per record, studio data panel, CLI `--data`/`--record` (ADR-0014, format v2, API 3) | done |
| 3.2a Elements | date/time bindings, flip, vertical text, Insert → Date/Time/Serial (ADR-0015) | done |
| 3.2b Elements | tables, borders, bundled icons, reading codes from images (ADR-0016) | done |
| 3.3 Editor | align, distribute, group, lock, snapping, grid, shortcuts, label colour (ADR-0017) | done |
| 3.4 Templates | starter set, local library and folders, PNG/PDF export, image and multi-image import (ADR-0018) | done |
| 3.5a Printing | print queue (pause, resume, cancel), local print history with reopen (ADR-0019) | done |
| 3.5b Device | label paper library, RFID barcode links and detection, calibration, auto-off and sound settings (ADR-0020) | done |
| 3.6a Print and codes | print offset, mirror, 180° output direction; Code 128 A/B/C and GS1-128 (ADR-0021) | done |
| 3.6b Shell | document tabs, recent files, rulers, printable-area overlay, rotate paper, clipboard, inch units (ADR-0022) | done |
| 3.6c Text and data | grow-to-fit text, max-length guard, font picker, drag-to-bind and element from column, rich text spans, image crop, linked mirror copies | done (ADR-0023) |

## Phase 4: Android shell + AppFunctions

- UniFFI Kotlin bindings, cargo-ndk builds, native BLE via `NativeBleLink`, Compose UI (editor,
  templates, print).
- AppFunctions subset (ADR-0011). The API version is re-verified at the start of the phase.
- **Exit criteria:** printing from a phone to the B21 Pro, and AppFunctions tested with
  `appfunctions-testing`.

## Phase 5: iOS shell + App Intents

- UniFFI Swift bindings (XCFramework), CoreBluetooth `NativeBleLink`, SwiftUI UI, App Intents and
  App Shortcuts.
- **Exit criteria:** printing from an iPhone to the B21 Pro; the intents run from Shortcuts.

## Phase 6: More models and vendors, packaging, releases

- **More printers:** remaining NIIMBOT variants (colour and two-colour, cutter models), then
  Phomemo, Marklife, generic ESC/POS, TSPL and ZPL drivers.
  - Each new model is a registry entry plus protocol tests.
- **Packaging:** cargo-dist for the CLI; signed and notarised macOS app, Windows MSI, Linux
  AppImage/deb (ADR at phase start); Play and App Store builds; tag-driven release workflow with
  SHA256SUMS.
