# ADR-0020: Label paper library, RFID detection, calibration and printer settings

- Status: proposed
- Date: 2026-11-15

## Context
Slice 3.5b of the feature matrix: the vendor app has a label paper library (browse by printer
series, search, custom sizes), recognises the loaded roll by its RFID tag, warns when it
differs from the template, calibrates the label sensor and changes device settings such as
the auto-off time. Its paper specs come from a vendor cloud catalogue, which Labely does not
use (ADR-0001). The printer's RFID tag gives a barcode, serial, label counts and a media type,
but no size (protocol.md §10).

## Decision
- **Bundled papers.** `crates/core/papers/builtin.json` is Labely's own table of common
  sizes (gap, rounded, transparent, black-mark labels and continuous rolls, 12–50 mm wide),
  with ids `std:<w>x<h>[-kind]`. It is not derived from any vendor list. Papers are filtered
  by printer model through its print width, not by vendor series.
- **User papers and links.** One JSON file set by the frontend
  (`Session::set_paper_file`; `<app-data>/papers.json` in the CLI, MCP server and studio)
  holds the user's papers (`my:<slug>`) and a map from RFID barcode to paper id. Linking is
  how a roll becomes known offline: the first time an unknown roll is loaded, the user (or an
  agent) links its barcode to a paper; afterwards it is recognised. Written through a temp
  file and a rename. Addressed by id, so not limited by the file roots.
- **Commands.** `paper.list`, `paper.apply` (undoable edit of the label size, corner radius
  and media), `paper.add`, `paper.delete`, `paper.link`, `paper.unlink` (library writes: not
  undoable, not journaled), and `paper.detect` (device: reads the RFID tag, looks the barcode
  up, reports `matches_label`). Detection never changes the document by itself.
- **Mismatch check.** `doc.check` `paper_mismatch` compares size (±0.5 mm) and media when
  the barcode is linked, otherwise media only as before. Its hint names the `paper.apply` id.
- **Calibration and settings.** `Driver` and `DeviceHub` gain `calibrate(media)`,
  `settings()` and `set_settings(patch)` with defaults that report "not supported".
  The NIIMBOT driver sends 0x8E with the label type and waits up to 20 s for 0x8F; reads the
  auto-off level (PrinterInfo 7) and the two sound settings (0x58 get) with a short timeout,
  leaving unanswered ones out; sets them with 0x27 and 0x58. The payload meaning of 0x8E and
  the B21 Pro answers are UNCERTAIN until checked on hardware (protocol.md open questions
  14–15). The simulator and FakePrinter implement all three.
- **Commands and gating.** `device.calibrate` (refused while print jobs are queued or
  printing; continuous paper is rejected), `device.settings`, `device.set_settings`.
  Calibration feeds paper, so it is gated by "Allow agents to print" like `print.*`;
  settings are not.
- **Studio.** Label → Label paper… opens the library (search, "only paper that fits", use,
  delete own papers, add a paper). Connecting a printer runs `paper.detect`; the printers
  panel shows the loaded paper with "Use" when the label differs, or "Link to a paper…" for an
  unknown barcode. Each connected printer has Settings… (auto-off, beeps, calibration);
  Print → Printer settings and calibration… opens it for the single connected printer.

## Consequences
- API version stays 3 (no release since it was bumped); the schema hash changed.
- Out of scope: measuring paper with 0x59/0x5A, RFID writing, ribbon RFID (0x1C), printer
  reset (0x28), language and print-speed settings, battery/firmware details on the settings
  page (device.status already reports battery), and app-level "always use the detected paper"
  defaults (the studio offers it instead of applying silently).
