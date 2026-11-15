# ADR-0024: Phase 3 close-out: device identity, font files, justify and studio gaps

- Status: proposed
- Date: 2026-11-15

## Context
The Phase 3 exit audit checked every Ph1–Ph3 row of the feature matrix against the code. About
75 rows were done; about 20 had gaps without a stated reason. A B21 Pro was available for
this slice, so device behaviour was checked on hardware rather than only on the simulator.

## Decision
- **BLE write fix.** `BleTransport::send` built its `tokio::time::timeout` outside the private
  runtime, which panicked on the first write from the CLI ("no reactor running"). The timer is
  now created inside `block_on`. Found on the B21 Pro; the simulator path does not use BLE.
- **Device identity.** At connect the NIIMBOT driver reads firmware (info 9), hardware (12)
  and serial (11), best effort. `DeviceInfo` gains optional `firmware`, `hardware` and
  `serial`. A B21 Pro reports firmware 3.12, hardware 3.01 and its serial as ASCII. The
  simulator answers the same way.
- **Speed and print mode.** On the B21 Pro, PrinterInfo 2 (speed) and 14 (print mode) answer
  with the "not supported" reply (0x00). Both stay unexposed; print mode only matters on
  thermal-transfer models, and speed is revisited with the models that answer (Ph6).
- **Justify.** `HAlign::Justify` maps to cosmic-text `Align::Justified`: wrapped lines fill the
  box and the last line stays left. Text and table cells accept it.
- **Font files.** `Renderer::add_font` and the `font.add` command load a TTF, OTF or TTC file
  (checked with `Ctx::check_path`, at most 64 MiB, magic bytes checked) for the session and
  return the families added. Documents keep only the family name. The studio now uses the
  renderer with system fonts (`desktop_services`) so its picker lists installed fonts. File →
  Add Font File remembers the file in the studio prefs and loads it at start. CLI, MCP and tests
  keep the bundled-only renderer, so their output does not depend on the machine.
- **Studio.** Save As (Cmd+Shift+S); rubber-band selection from empty canvas (touching,
  visible, unlocked elements; Shift adds); a solid/dashed/dotted line-style control for
  `dash_mm`; table cell alignment, size, invert, row height, column width and Clear text; an
  element palette (click to add, drag onto the canvas to place); print dialog copies-from-column
  and an as-printed preview (mirror and 180° applied); XLSX sheet and header-row controls in the
  data panel; printer battery, lid, labels left, firmware and serial instead of raw JSON, the
  connected printer and the number of active jobs in the status bar; connect troubleshooting
  tips with Try again. Language and label colour are saved in the prefs.

## Consequences
- The schema hash changed (`DeviceInfo` fields, `HAlign::justify`, `font.add`). API version
  stays 3: there has been no release since it was bumped. `.labely` files are unchanged.
- A document that uses an added or installed font renders with the default font on machines
  without it; `doc.check` already reports that.
- The remaining gaps have a stated reason and a target phase in the feature matrix
  ("Phase 3 exit status").
