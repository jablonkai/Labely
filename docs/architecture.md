# Labely architecture

Status: proposed (Phase 0). Decisions are recorded in [`docs/adr/`](adr/). Conventions follow
[`prior-art.md` §4](prior-art.md#4-conventions-labely-adopts). Where this document departs from
them it says so. Protocol facts are in [`protocol.md`](protocol.md) (NIIMBOT) and
[`protocol-languages.md`](protocol-languages.md) (other vendors), models in
[`printers.md`](printers.md), and the parity target in [`feature-matrix.md`](feature-matrix.md).

## 1. Goals and non-goals

- **Goals.**
  - Free and open source (MIT OR Apache-2.0), works fully offline, no accounts, ads or telemetry.
  - One Rust engine drives every surface: GUI, CLI, MCP, Android and iOS.
  - Every document change is a named command with a JSON schema. A command is one undo step and
    returns a structured result.
  - Feature parity with the NIIMBOT desktop app 4.2.7, plus data merge, a print queue,
    multiple printers, an open JSON format, and EN/HU localisation.
- **Non-goals.**
  - Vendor cloud, vendor accounts or VIP logic.
  - Vendor firmware updates, which depend on the vendor's cloud.
  - Consumable authenticity checks.

## 2. Workspace layout

```
Cargo.toml                 workspace; edition 2024; license MIT OR Apache-2.0; Cargo.lock committed
rust-toolchain.toml        pinned stable (1.100.0 at time of writing), MSRV 1.95 (egui 0.37)
crates/
  protocol/   labely-protocol    pure encode/decode, no I/O, no async (NIIMBOT, ESC/POS, Phomemo, Marklife, TSPL, ZPL)
  core/       labely-core        document model, command registry, Session, undo/redo, batch, templates, data merge, check rules
  render/     labely-render      Document → grayscale → 1-bit Bitmap1 (text, barcodes, 2D codes, images, shapes, dithering)
  transport/  labely-transport   Transport trait; ble (btleplug), serial/usb, memory, callback (FFI)
  printers/   labely-printers    Driver trait, model registry (data files), DeviceHub, FakePrinter, NIIMBOT simulator
  mcp/        labely-mcp         rmcp server handler; Streamable HTTP (studio) + stdio (headless/bridge)
  cli/        labely-cli         binary `labely`
  studio/     labely-studio      egui app; lib target (for egui_kittest) + bin target
  ffi/        labely-ffi         UniFFI proc-macro bindings → Kotlin, Swift
  testkit/    labely-testkit     dev-only: golden helpers, fixture loaders, sim printers
  i18n/       labely-i18n        Fluent bundles (EN, HU) for studio and CLI; locales embedded (ADR-0013)
xtask/                           ci, layers, bless, schema-hash, ui-commands, i18n-check
mobile/android/                  Kotlin + Compose shell, AppFunctions
mobile/ios/                      SwiftUI shell, App Intents
fixtures/                        captured packet traces (.pklg/.btsnoop → normalised JSON), golden PNGs
docs/                            specs, ADRs, agent docs
AGENTS.md                        how agents build, test and drive Labely
```

**Departure from prior-art §4.1.** Prior art puts thin binaries under `apps/`. Here the CLI and
studio are single crates with both `lib` and `bin` targets, so the crate names match the
project brief (`labely-cli`, `labely-studio`). The studio lib target is still testable
with kittest.

### 2.1 Dependency layers (enforced by `cargo xtask layers`)

| Layer | Crates | May depend on |
|---|---|---|
| L0 | `labely-protocol`, `labely-core`, `labely-i18n` | std, serde, schemars (i18n: fluent); nothing in this workspace (all three are independent) |
| L1 | `labely-render` (→ core), `labely-transport` (no workspace deps) | L0 |
| L2 | `labely-printers` (→ protocol, transport, render, core) | L0–L1 |
| L3 | `labely-mcp`, `labely-ffi` | L0–L2 |
| L4 | `labely-cli`, `labely-studio` | L0–L3 |

GUI crates (`egui`, `eframe`, `winit`, `rfd`) are forbidden below L4, and `btleplug` outside
`labely-transport`.

**Departure from prior-art §4.1 item 3.** There, transport sat above printers. Here a driver owns a
transport, so `printers` sits above `transport`. The transport stays a plain byte pipe with no
protocol knowledge.

**Dependency inversion for print and preview.** `labely-core` defines two service traits:

- `Renderer` (`render(doc, page, opts) -> Bitmap1`, `decode_barcodes(bitmap)`), and
- `DeviceHub` (`list`, `connect`, `status`, `submit(job)`, `cancel`).

`Session::new(services)` receives them. `print.*`, `device.*`, `render.preview` and `doc.check`
are ordinary core commands that call those traits. `labely_printers::default_services()` wires the
real implementations, and the testkit wires fakes. The crate list stays as in the brief and core
stays testable with no I/O.

## 3. Core model

### 3.1 Document (`.labely`, versioned JSON; ADR-0008)

```
Document { format_version, id, meta{name, created, modified}, label: LabelSpec,
           pages: Vec<Page> (usually 1), data: Option<DataSource>, serials: Map<name, SerialSpec>,
           variables: Map<name, String> }   // data and serials: format v2, ADR-0014
LabelSpec { width_mm, height_mm, corner_radius_mm, shape: Rect|Round|Custom, gap: Gap|BlackMark|Continuous|Transparent,
            margins_mm, print_direction, printer_hint: Option<ModelRef>, rfid_hint: Option<PaperRef> }
Page { background: Option<ImageRef>, elements: Vec<Element> }        // z-order = vec order
Element { id: ElementId(u64), name, frame{x_mm,y_mm,w_mm,h_mm,rotation_deg}, locked, hidden, group: Option<GroupId>,
          kind: Text|Barcode|Code2d|Image|Rect|Ellipse|Line|Table|Date|Serial|Icon|Border }
Binding: any string property may be a template "{{column}}", "{{serial}}", "{{now:%Y-%m-%d}}"
```

- Units are mm (f64) everywhere in the model. Conversion to dots happens only in `labely-render`.
- Ids are stable `u64` values, never indices.
- Children are behind `Arc`, so cloning a snapshot is cheap.
- Assets (images, fonts) are content-addressed blobs embedded in the file (base64) or kept in a
  sidecar `.labelyz` zip for large ones (ADR-0008).

### 3.2 Commands and Session (ADR-0004)

- **Command trait.** As in prior-art §4.2: `const ID`, typed `Params`/`Output` (serde + schemars
  1.x, `deny_unknown_fields`), `CommandMeta { label, menu, shortcut, kind: Query|Edit|Device|Ui,
  undoable, journal }`, `enabled`, `run`.
- **Registration.** Commands are registered from an explicit list in `core/src/registry.rs`,
  where `Entry::in_menu(menu, shortcut)` also places them in the studio's generated menus.
  Zoom and panel toggles are studio view state, not commands. `selection.set` is the core
  `Ui` command (it is never undone or journaled).
- **Policy.** `Session` carries a `Policy {file_roots, allow_print}` (ADR-0012). The MCP
  server and the studio's agent calls use a restricted policy; the CLI does not.
- **Single entry point.** `Session::execute(id, params: Value) -> Envelope`. It is the only
  mutation path for the studio, CLI, MCP and FFI.
- **Envelope.**
  - Success: `{ok:true, result, changed, created, warnings}`.
  - Failure: `{ok:false, error:{code, message, param?, accepted?, hint?}}`, with the stable codes
    from prior-art §4.2 item 9.
- **API version.** `API_VERSION`, with a schema-hash test that fails until the version is bumped.
- **Undo.** One `Arc<Document>` snapshot per `Edit` command (cap: 200 steps or a byte budget).
  - `coalesce` merges slider drags and typing into one step.
  - `*.begin/preview/commit` commands make a canvas gesture one step.
  - A `catch_unwind` guard restores the snapshot taken before a panicking command.
- **Batch.** `command.batch {steps, atomic=true, stop_on_error=true, label, terse}`.
  - Atomic by default: params are checked first, then the steps run, and any failure restores the
    state from before the batch. A successful batch is one undo step.
  - Later steps can use earlier results through references: `$N.path`, `$last.path` (escape `$$`).
  - `print.*` and `device.*` are refused inside an atomic batch. Agents print after the batch
    instead.
  - Limits: 128 steps, 4 MiB of results.
- **Journal.** Every `journal` command is recorded as `{id, params}`. The journal gives CLI
  scripts, macros and replay tests.

### 3.3 Initial command catalogue (grows to parity in Phase 3)

| Area | Commands |
|---|---|
| doc | `doc.new`, `doc.open`, `doc.save`, `doc.close`, `doc.get`, `doc.set_label`, `doc.check` |
| element | `element.add_text`, `.add_barcode`, `.add_code2d`, `.add_image`, `.add_shape`, `.add_line`, `.add_table`, `.add_date`, `.add_serial`, `.add_icon`, `.add_border`, `.set_props`, `.move`, `.resize`, `.rotate`, `.delete`, `.duplicate`, `.reorder`, `.align`, `.distribute`, `.group`, `.ungroup`, `.lock` |
| selection / history | `selection.set`, `selection.get`, `history.undo`, `history.redo`, `history.journal` |
| data | `data.attach_csv`, `data.attach_xlsx`, `data.bind`, `data.preview_record`, `variable.set` |
| template | `template.list`, `template.instantiate`, `template.save_as` |
| render | `render.preview` (PNG of the print raster), `render.export` (PNG/PDF) |
| device / print | `device.scan`, `device.connect`, `device.disconnect`, `device.status`, `device.read_paper` (RFID), `print.submit {copies, density, range, records}`, `print.queue`, `print.cancel` |
| meta | `command.list`, `command.batch` |

### 3.4 `doc.check` rules

The command returns `[{rule, severity, element_id?, message, hint}]`. Rule ids are listed in
[`docs/agents/tools.md`](agents/tools.md#doccheck-rules). Paper size versus RFID uses the label
paper library (ADR-0020): sizes are compared when the RFID barcode is linked to a paper,
otherwise only the media type.

- text overflow or text auto-shrunk below 5 pt at the device dpi
- element outside the printable area or margins
- barcode or QR that does not round-trip: render at device dpi, decode with `rxing`, compare
- barcode module narrower than 2 dots, or quiet zone too small
- label size different from the paper the printer reported over RFID
- document dpi or width wider than the selected model's head
- binding that references a missing data column, or an empty value in some record
- image with low contrast after dithering

## 4. Rendering (`labely-render`, ADR-0007)

- **Pipeline.** Document plus record → layout in mm → rasterise to 8-bit gray at the model dpi
  (`tiny-skia`) → per-element conversion to 1-bit (threshold, Floyd–Steinberg, Atkinson,
  ordered/Bayer) → `Bitmap1`.
- **Bitmap1 orientation.** `Bitmap1` is in label orientation. The printer driver rotates it for
  the model's print direction and fits it to the head ([protocol.md §7.1](protocol.md)).
- **Text.** `cosmic-text` (shaping, fallback, wrapping, auto-shrink). Bundled OFL fonts make
  goldens deterministic, and system fonts are optional. Vertical and curved text are added in
  Phase 3.
- **Codes.** `rxing` encodes and decodes 1D codes (Code128 A/B/C, Code39, Code93, EAN-8/13,
  UPC-A/E, ITF, Codabar, GS1-128), QR, PDF417 and DataMatrix. Code modules are snapped to whole
  dots, never anti-aliased.
- **Images.** The `image` crate decodes PNG, JPEG, BMP and WebP. `imageproc` provides
  brightness, contrast and invert.
- **Output.** `Bitmap1` (packed MSB-first rows, width in dots) plus a gray preview. Both are
  exportable as PNG.
- **Deterministic.** The same input gives byte-identical output, which is what the golden tests
  rely on.

## 5. Printing stack

```
print.submit ─▶ DeviceHub ─▶ PrinterSession (worker thread per printer, job queue)
                               │  Driver (trait)          ◀── model registry entry
                               │    NiimbotDriver{variant}  StreamDriver{language}  FakePrinter
                               │  uses labely-protocol encoders
                               ▼
                             Transport (trait): Ble(btleplug) | Serial(USB CDC) | Tcp | Callback(FFI) | Sim
```

- **Transport.**
  ```
  trait Transport: Send {
      fn send(&mut self, bytes) -> Result<()>;
      fn recv(&mut self, timeout) -> Result<Vec<u8>>;
      fn mtu(&self) -> usize;
      fn info(&self) -> TransportInfo;
      fn close(&mut self);
  }
  ```
  - Framing and request/reply pairing live in the driver, through the protocol crate's
    `Program`, not in the transport.
  - BLE chunking uses `mtu()`.
- **Driver.**
  ```
  trait Driver {
      fn connect(&mut self, t) -> Result<DeviceInfo>;
      fn status(&mut self) -> Result<Status>;
      fn read_paper(&mut self) -> Result<Option<PaperInfo>>;
      fn print(&mut self, job: &PrintJob, progress: &dyn Fn(Progress)) -> Result<()>;
      fn cancel(&mut self);
  }
  ```
  `PrintJob { pages: Vec<Bitmap1>, copies, density, label_type, … }`.
- **Model registry.** The data files are `crates/printers/models/<vendor>.json`, embedded with
  `include_str!` and extendable by user files. One entry is one model:
  ```json
  { "id": "niimbot.b21_pro", "vendor": "niimbot", "name": "B21 Pro", "match": {"model_codes": [785], "ble_name_prefix": ["B21"]},
    "driver": "niimbot", "task": "d110m_v4", "dpi": 300, "head_dots": 591, "max_width_mm": 50,
    "print_direction": "top", "density": {"min":1,"max":5,"default":3}, "label_types": ["gap","black_mark","continuous","transparent"],
    "rfid": true, "transports": ["ble","usb"] }
  ```
  The model code (PrinterInfo 0x40, type 8) is authoritative and the BLE name only filters scans ([printers.md](printers.md#how-models-are-identified)). `task` is one of `d110`, `b1`, `d110m_v4` or `auto` (picked from the protocol version); other variants listed in protocol.md §5.2 are recorded but not yet implemented.
  Printers of other vendors do not report a model. `driver` and `task` pick their language ([protocol-languages.md](protocol-languages.md)), and the model comes from the BLE name or from `?model=<id>` on the address (ADR-0037).
- **FakePrinter.** A `driver: "fake"` model writes each page as `<out>/<job>-<page>.png` plus a
  `job.json` (density, copies, label type). It is the default in tests, the CLI `--printer
  fake:<dir>` and demo mode.
- **Simulator.** `labely-printers::sim` is a stateful NIIMBOT protocol simulator on top of
  `labely-transport::memory`. It answers handshake, status and RFID, models the B21 Pro
  quirks, and reassembles the raster, so driver tests run real byte streams without hardware
  (`sim:<model id>` address). It lives in `printers`, not `transport`, because it needs the
  protocol crate. Captured vendor traces (`fixtures/`) are replayed against encoders and decoders.
  `labely-printers::stream_sim` does the same for the stream languages of the other vendors.
- **Queue and multiple printers.** The `DeviceHub` keeps one `PrinterSession` per connected
  printer. Each session has a FIFO queue with per-job progress events (`device.*` notifications,
  forwarded over MCP as progress). Batches over data records are expanded lazily, one record at a
  time.

## 6. BLE placement (ADR-0006)

- **Desktop and CLI.** `labely-transport::ble` uses btleplug 0.13.x (CoreBluetooth, WinRT, BlueZ).
- **Android and iOS.** BLE lives in the native shell (Kotlin `BluetoothGatt`, Swift
  CoreBluetooth). It implements the UniFFI callbacks `NativeBle { scan, connect }` and
  `NativeBleLink { write, mtu, name, disconnect }`, and pushes notifications into a `BleNotifySink`.
  `labely-transport::callback` adapts them to `Transport`, and `Hub::with_native_ble` routes
  `ble:` addresses through them (ADR-0025).
- **Why.** btleplug on Android needs JNI class-loader setup, R8 keep rules and bundled Java
  classes, and has open MTU issues (#503, #504). Native BLE keeps permissions and the background
  lifecycle on the platform's terms, and the protocol stays in Rust.

## 7. Frontends

- **CLI (`labely`, clap 4.6).**
  - `labely run <file> --cmd <id> --params <json>`
  - `labely run --script steps.json`
  - `labely commands [--json]`
  - `labely render <file> -o out.png [--record N]`
  - `labely check <file>`
  - `labely print <file> --printer <ble:NAME|usb:PORT|fake:DIR> [--data x.csv]`
  - `labely devices`
  - `labely mcp [--headless|--connect]`
  - `labely capture <trace>`

  Output is JSON envelopes with `--json`, or human-readable text otherwise.
- **MCP (`labely-mcp`, rmcp 3.10, spec 2026-07-28; ADR-0005).**
  - **Studio:** Streamable HTTP on `127.0.0.1:7878/mcp`, loopback only. It checks `Origin`,
    requires a bearer token from `<app-data>/mcp-connection.json` (mode 0600), and can be turned
    off.
  - **`labely mcp` (stdio):** forwards to the running studio if one is listening, otherwise it
    serves an in-process headless Session.
  - **Tools:** `command_list`, `command_run`, `command_batch`, `document_get`, `document_new`,
    `document_open`, `document_save`, `render_preview` (an `image/png` of the print raster plus
    dims and dpi), `check`, `device_list`, `device_status`, `print`, `undo`, `redo`.
  - **Gating:** printing from an agent is off until the user allows it in the studio, or
    headless mode runs with `--allow-print`. File access is limited to the configured roots.
- **Studio (`labely-studio`, egui/eframe 0.37).**
  - **State:** `LabelyApp { session, ui: UiState }`. Panels read the session directly; the
    canvas shows the exact print raster (cached per document `Arc`).
  - **Edits:** every edit goes through `app.run(id, params)`. MCP calls reach the UI thread over a
    channel with a deadline and are deferred while a gesture or dialog is open.
  - **Layout:** canvas, layers, properties, data, printer/queue panels and a command palette.
  - **Generated UI:** menus and shortcuts come from `CommandMeta`.
  - **Tests:** egui_kittest interaction tests through the AccessKit tree, without image
    snapshots, because CI runners have no GPU.
- **FFI (`labely-ffi`, UniFFI 0.32, proc-macro mode).**
  - **Object:** `LabelyEngine` with `execute(id, params_json) -> envelope_json`,
    `commands_json()`, `render_png(doc_id, dpi, page)` and `set_allow_print`. A device-event
    callback is added with the Compose UI (ADR-0025).
  - **BLE:** the `NativeBle` and `NativeBleLink` callbacks for native BLE.
  - **Bindings:** generated Kotlin and Swift by `cargo xtask bindings`, not committed.
- **Android shell** (Kotlin 2.4, Compose BOM 2026.09, AGP 9.5).
  - **Scope:** UI and platform glue only. App minSdk is 26.
  - **AppFunctions (`androidx.appfunctions` 1.0.0-rc01, Android 16 / API 36+, behind
    `@RequiresApi(36)`):** `createLabelFromText`, `createLabelFromTemplate`, `listTemplates`,
    `listPrinters`, `getPrinterStatus`, `printLabel`. Each one maps 1:1 onto a command or batch
    through `LabelyEngine.execute` (ADR-0011).
- **iOS shell** (SwiftUI, min iOS 18).
  - **App Intents:** the same six intents, with `AppEntity` for labels, templates and printers
    and `AppShortcutsProvider`, all mapped onto `execute` through `LabelyKit.Assistant`
    (ADR-0033).
  - **Later:** `LongRunningIntent` for printing once the app targets iOS 27.

## 8. Localisation (ADR-0009)

- **Rust strings:** Fluent (`fluent-bundle` 0.16) for the studio and CLI messages, in
  `locales/{en,hu}/*.ftl`.
- **Error messages:** command error messages are English and stable for agents. A localised
  `message_l10n` is added to the envelope for display.
- **Mobile:** the shells use platform string resources. `xtask i18n-check` checks key parity
  between EN and HU across all three sets.

## 9. Quality and CI

- **Static checks:** `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings`, layers check, schema-hash check.
- **Lints:** `unsafe_code` is denied except in `ffi` and `transport`. Library crates deny
  `unwrap` and `expect`.
- **Tests:**
  - unit tests per crate
  - `tests/it` integration tests
  - golden 1-bit PNGs (exact comparison; refresh with `LABELY_BLESS=1 cargo xtask bless`)
  - packet tests from captured fixtures and encoder byte-goldens
  - replay of the sim transport
  - schema snapshots with `insta`
  - kittest UI snapshots
  - fuzzing the protocol decoders (nightly job)
- **GitHub Actions:** `cargo xtask ci` on macOS, Linux and Windows; feature matrix
  (`--no-default-features`, `--all-features`); FFI builds for Android (cargo-ndk) and iOS from
  Phase 4/5; tag-triggered releases in Phase 6.
- **Dependency versions:** checked online on 2026-11-14 (table below) and re-checked when each
  crate is added.

| Crate | Version | | Crate | Version |
|---|---|---|---|---|
| eframe/egui | 0.37.0 | | rmcp | 3.10.0 |
| btleplug | 0.13.5 | | schemars | 1.2.3 |
| serialport | 4.10.1 | | uniffi | 0.32.2 |
| tokio | 1.54.1 | | tiny-skia | 0.12.0 |
| cosmic-text | 0.19.0 | | rxing | 0.9.4 |
| image / imageproc | 0.25.10 / 0.27.0 | | calamine | 0.38.0 |
| csv | 1.4.0 | | clap | 4.6.7 |
| insta | 1.49.0 | | fluent-bundle | 0.16.0 |
| thiserror / anyhow | 2.0.21 / 1.0.104 | | cargo-dist | 0.34.0 |

## 10. Security and privacy

- **Network:** none except loopback MCP and the user's printers. No update checks without opt-in
  (Phase 6), and no telemetry.
- **MCP:** loopback bind, `Origin` check, bearer token, print gating, file-root gating.
- **Parsing:** protocol decoders are bounded and fuzzed. File and XLSX imports have size limits.

## 11. Risks

| Risk | Mitigation |
|---|---|
| B21 Pro variant details (page-size length, PrintStart bytes, heartbeat layout) are unresolved | Phase 1 begins with a PacketLogger trace of the vendor app printing to the B21 Pro, used for behaviour observation only, plus sim tests. Fallbacks follow the variants in protocol.md §5.3. |
| rmcp Streamable HTTP may only answer with SSE | Clients support SSE; otherwise a small JSON-only endpoint behind the same handler (prior-art §4.6) |
| AppFunctions is a release candidate and needs API 36 | Isolated in one Android module behind `@RequiresApi(36)` and re-checked at Phase 4 |
| Text-rendering differences between platforms break goldens | Bundled fonts only in goldens; cosmic-text configured with no system fallback in tests |
