---
name: add-printer
description: Add support for a new label printer model, model family or vendor to Labely, clean-room — spec with cited evidence first, then the encoder in labely-protocol, the registry entry and driver in labely-printers, the simulator, tests, docs and an ADR. Use whenever the user wants Labely to print on a printer it does not list yet, a new NIIMBOT model, a new Phomemo/Marklife/ESC/POS/TSPL/ZPL model, a new vendor or printer language, or a missing print task or dialect — even if they only name the device ("support the Phomemo M04S", "add the Marklife P50", "can Labely print on a Brother QL?"). Hungarian triggers too: "támogasd az X nyomtatót", "adj hozzá egy új nyomtatót", "új nyomtató modell", "új gyártó", "legyen támogatott a(z) X címkenyomtató", "nyomtasson a(z) X-re is".
---

# Add a printer to Labely

Labely talks to printers it learned about from written specs, never from vendor code. That is
what keeps the project legally clean (ADR-0001), so the order of work matters as much as the
code: **evidence → spec → code → tests → docs → ADR**. Read `docs/adr/0001-record-decisions-and-clean-room.md`,
then ADR-0036 (NIIMBOT variants) and ADR-0037 (other vendors) — they are the two worked
examples of this skill, and the commits `08caa87` and `134e60e` show the real file sequence
(`git show --stat 134e60e`).

## 1. Classify the request

Decide which of these it is, because the amount of work differs a lot:

| Case | Example | Touches |
|---|---|---|
| A. New model, existing language/task | another M110-family Phomemo, a NIIMBOT with a known task | spec row + registry JSON + tests |
| B. New task/dialect of a known vendor | Marklife P50 (compressed dialect, protocol-languages.md §4.2) | + encoder/decoder, `Language` variant, driver, simulator |
| C. New vendor or printer language | a vendor not in `crates/protocol/src/` | + new protocol module, sources table, maybe transport, ADR |
| D. NIIMBOT protocol work | a new print task or packet | `docs/protocol.md`, `crates/protocol/src/niimbot/`, `NiimbotDriver`, `sim.rs` |

Case A usually needs no ADR; B, C and D do (use the `new-adr` skill).

## 2. Clean-room rules (non-negotiable)

These come from ADR-0001. Explain them to the user if a request pushes against them.

- **Spec first.** Facts go into `docs/protocol.md` (NIIMBOT), `docs/printers.md` (NIIMBOT
  models) or `docs/protocol-languages.md` (every other vendor) *before* any code. Every byte
  and every model fact cites a source key, so each behaviour in code traces back to a spec
  section.
- **Never copy** vendor code, decompiled output, assets, templates, fonts, icons or UI strings.
  Write the spec in your own words; quote only short identifiers.
- **No vendor cloud.** Do not call or design around vendor cloud APIs (and no third-party
  rendering services either — ADR-0044 rejected Labelary for that reason).
- **Never touch licence, VIP or consumable-authenticity checks** — not to investigate, not to
  bypass. If a printer refuses non-genuine paper, document that it does and stop there.
- **Open-source projects are documentation, not code.** niimprint, niimbluelib and NiimBlue
  (MIT) are cross-checked and cited (`O:np`, `O:nbl`, `O:nb` in protocol.md §0). For other
  vendors, projects are cited as facts only, even GPL ones (protocol-languages.md §0 table has a
  "Reuse in Labely" column — fill it). Do not paste their code into Labely or into your
  context-to-code path; restate the behaviour.
- **Captures** of the vendor app talking to the user's own printer are fine for observing
  behaviour; store them as normalised fixtures.
- **Mark uncertainty.** A fact with one source or conflicting sources is *unverified* in the
  spec. Do not guess a head width or dpi: a model is listed only when its head width and job
  layout have a source (ADR-0037). Leave it out and say why (see §8 "Left out because…").

### Gathering evidence

Prefer vendor manuals (as for ESC/POS, TSPL, ZPL) and permissively licensed OSS docs. When a
claim depends on a shipped binary or app package, the repo's `reverse-engineer-anything`
skill (`.claude/skills/reverse-engineer-anything/`) can gather it — but its output goes **only
into the spec documents** as cited observations, never into product code, and the never-do list
above still applies to what you look at.

## 3. Write the spec

- **Other vendors:** `docs/protocol-languages.md`. Add sources to the §0 key table (key, source,
  licence, reuse). Add or extend a language section with the byte table of one page, status
  request/reply and registry tasks (follow §3.1 Phomemo M02 as the template). Add the BLE
  service/characteristics and name format to §7, the model rows to §8 (id, driver/task, dpi,
  head dots, media, source), and open questions to §9. Keep the header line "Nothing in this
  file has been tried on a printer" truthful.
- **NIIMBOT:** `docs/protocol.md` for packets and tasks, `docs/printers.md` model table (columns
  per its legend; OSS-only models go in their own subsection).

## 4. Code

Library code denies `unwrap`, `expect` and `panic` (AGENTS.md) — return errors.

**Encoder — `crates/protocol/src/`** (pure: no I/O, no async; deps are only serde/thiserror).
- New language: a module `<lang>.rs` and `pub mod <lang>;` in `lib.rs`. It builds the bytes of
  one page from a `RasterView` and has a `decode` for the bytes it writes (the simulator and
  tests need it). Return the owned `Raster` from decoders. Look at `tspl.rs`/`marklife.rs` for
  the size of a typical module and its unit tests (round-trip and `decode_errors`).
- Raster only: send the rendered 1-bit page; never printer fonts or native barcodes, so the
  print matches the preview. Geometry follows protocol-languages.md §1.
- NIIMBOT: `niimbot/job.rs` (`PrintTask`, `from_name`, `supports`), `rows.rs`, `packet.rs`.

**Driver — `crates/printers/src/`.**
- `stream.rs`: add a `Language` variant, its arm in `Language::of` (matches `(driver, task)`
  from the model), and its arms in `steps` (page bytes, media, density) and `status`. Stream
  printers do not confirm pages; status is asked before the job and a silent printer counts as
  ready after 500 ms.
- `stream_sim.rs`: arms in `pages` (decode back) and `status_reply`.
- `hub.rs::driver_for` picks `StreamDriver` whenever `Language::of` matches, otherwise
  `NiimbotDriver` — normally no change. NIIMBOT work goes in `niimbot.rs` and `sim.rs`
  (`SimConfig::for_model`). `fake.rs` (PNG output) is model-agnostic.
- `driver.rs` only changes if the job request itself needs a new field (as ADR-0036 did) —
  that likely means a command schema change; see AGENTS.md "Schema changes".

**Registry — `crates/printers/models/<vendor>.json`.** One entry per model, fields as in
`registry.rs::Model` (`deny_unknown_fields`): `id` = `<vendor>.<model>`, `driver`, `task`,
`dpi`, `head_dots` (multiple of 8 for non-NIIMBOT), `max_width_mm`, `max_length_mm`, `media`,
`print_direction`, `density` (clamped range per language, ADR-0037), `transports`, optional
`model_codes`, `ble_name_prefixes`, `rfid`, `gray`, `red`. A new vendor file must also be added
to `BUILTIN` in `registry.rs`. Note: `guess_by_name` matches the advertised name against the
model `name` (normalised, longest wins); `ble_name_prefixes` only filter scans.

**Transport — `crates/transport/src/`** only if the link is new: `gatt.rs::select_link` (BLE
service fallbacks, e.g. the `FF00`/`FF02` split write/notify), `tcp.rs` (port 9100),
`serial.rs`. Most stream printers need none.

## 5. Tests

- Unit tests in the protocol module (encode → decode round trip, decode errors).
- `crates/printers/src/registry.rs` tests: `every_builtin_file_parses_and_names_a_driver` runs
  over every file; add the BLE name to `other_vendors_are_recognised_by_name` (or the NIIMBOT
  equivalents).
- `crates/printers/tests/it.rs`: `every_stream_model_prints_on_the_simulator` picks up new
  stream models automatically (raise its `>= 15` floor); `every_registered_model_prints_on_the_simulator`
  asserts the exact NIIMBOT count (66) — update it and its comment. Add a byte-level test like
  `stream_job_bytes_follow_the_request` for a new language.
- Run `cargo test -p labely-protocol -p labely-printers`, then `cargo xtask ci`. No hardware is
  needed; if the user has the device, a real print is the final check. Otherwise say plainly
  that nothing ran on hardware.

## 6. Docs and ADR

- `docs/feature-matrix.md`: update the row or exit-status table it affects.
- `docs/architecture.md` if a language, driver or transport is new; `docs/agents/tools.md` if
  addresses or `device.models` behaviour change; `crates/cli/src/main.rs` address help if a
  scheme changes; `docs/roadmap.md` progress/Later items (e.g. remove "Marklife P50" from Later).
- ADR for cases B–D (use `new-adr`). Model it on ADR-0037: Context (what the repo lacked),
  Decision (spec first, encoder, driver, registry, simulator — each bullet bold-led),
  Consequences (always state what has *not* run on hardware, schema/`API_VERSION` impact,
  model count change).

## 7. Mobile and FFI impact

Say it explicitly in the ADR, even if the answer is "none yet". The mobile shells scan with the
registry's `ble_name_prefixes` through the FFI hub (`hub.rs::scan_native_ble`), so a new BLE
name shows up automatically. But `mobile/android/app/src/main/kotlin/org/labely/app/ble/Gatt.kt`
and `mobile/ios/LabelyKit/Sources/LabelyKit/Gatt.swift` still want one characteristic for both
directions, so split-characteristic printers (Phomemo, Marklife) do not connect on mobile
(ADR-0037, roadmap "Later"). If you change them, test with `./gradlew` and `swift test`
per AGENTS.md, and keep `Gatt.kt`, `Gatt.swift` and `gatt.rs` agreeing.

## Final report to the user

List: sources used (keys), spec sections added, files changed, tests run, what is unverified,
and whether anything ran on real hardware.
