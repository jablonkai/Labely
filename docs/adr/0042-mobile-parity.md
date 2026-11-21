# ADR-0042: Mobile parity

- Status: proposed
- Date: 2026-11-21

## Context
ADR-0027 and ADR-0032 left seven editor features out of the phone shells so that both would get
them from one pass over `design/labely-mobile.figcraft`: data merge (M4), the barcode screen
(M6), export (M12), printer settings, pinch zoom, resize handles (M2) and images. The engine
already has every command they need.

## Decision
- **No engine change.** Both shells use `LabelyEngine.execute` only. No FFI method, command or
  schema changed.
- **Files stay inside the data directory.** The engine of a phone may read and write only its
  data directory (ADR-0012). So:
  - a CSV is read by the shell and attached as text (`data.attach_csv {text}`);
  - a spreadsheet is copied to `imports/`, attached by path and the copy removed;
  - a picture is scaled to at most 1024 px and sent as `data_base64`;
  - an export is written to `exports/` (emptied first) and then handed to the share sheet or
    the system save picker.
- **Data merge.** A Data tab in the editor toolbar shows the source, the record stepper and the
  rows (50 per `data.get`). With data attached the canvas is `render.preview {record}`.
  Text, barcodes and 2D codes get a Binding control (`data.bind`). There is no unbind command:
  "None" replaces `{{column}}` with the value of the shown record. The print sheet has
  Records: Current or All; the default is the shown record, not the engine's default of all.
- **Barcode sheet.** Symbologies of the element's own kind, the value, the human-readable
  text and the binding. `element.set_props` takes any code data and the engine reports a bad
  value only as `encode_failed` in `doc.check`. So a new value or symbology is first tried on
  a scratch document and applied only when it encodes. A rejected value never reaches the
  document or its undo history. A bound value is not checked, because it depends on the record.
- **Export.** PNG or PDF, 203, 300 or 600 dpi, the shown record or all. A PDF has one page per
  record; PNG gives one file per record.
- **Printer settings.** Auto-off and the two sounds, only the ones `device.settings` reports,
  with calibration in the same sheet.
- **Canvas.** Pinch zooms from fit to 8×, two fingers pan, the badge shows the percent and
  returns to fit. The selected element has eight handles (a line has two; a locked or rotated
  element none). A resize is one `element.set_props` with the frame, so it is one undo step.
  iOS uses UIKit gesture recognisers behind the canvas, because SwiftUI has no two-finger pan.
- **Left out, with reasons.**
  - *SVG, transparent background and the 1-bit switch of M12.* The engine exports PNG and PDF
    of the print raster only.
  - *Binding an image.* `data.bind` refuses image elements.
  - *The CSV file name after reopening.* `data.attach_csv {text}` stores no origin.
  - *A sharper canvas when zoomed.* It still shows the 203 dpi print raster.
  - *RFID paper on the phone* (ADR-0041 is studio only).
- **The check digit is computed in the shell**, with the same mod-10 rule as the studio
  inspector, because the engine does not return the completed EAN or UPC value.

## Consequences
- Android: 29 new unit tests on the host engine (`EditorLogicTest`, `EditorStateTest`), and a
  `FileProvider` for sharing. iOS: a `ParityTests` suite in LabelyKit. No new dependency.
- Nothing ran on a phone, an emulator or a simulator. Gestures, the system pickers, sharing
  and the layouts against M2, M4, M6 and M12 are open hand checks on both platforms.
