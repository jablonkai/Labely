# ADR-0043: NiimBlue import

- Status: proposed
- Date: 2026-11-22

## Context
NiimBlue is the largest open-source NIIMBOT client. Its users keep their labels as JSON
exports, and those labels could not be opened in Labely (issue #43). The format is not
documented. The mapping below comes from NiimBlue's MIT-licensed source (v2026.10.1:
`src/types.ts`, `src/utils/file_utils.ts`, `src/fabric-object/`) and from niimbluelib's image
encoder. No code was copied (ADR-0001).

A NiimBlue export is `{label, canvas: {version, objects}, title?, timestamp?, thumbnailBase64?,
csv?: {data}}`, or a JSON array of them. `canvas` is Fabric.js 7 JSON in printer dots. The file
has no format version and no dpi.

## Decision
- **Read-only, through `doc.open`.** The file content decides the format: JSON without
  `format_version` that has `canvas` and `label` is NiimBlue. An imported document opens with no
  path and unsaved, so `doc.save` never writes over the export. There is no export to NiimBlue.
- **`dpi` parameter.** `doc.open` takes an optional `dpi` (100–1200, default 203 = 8 dots/mm,
  NiimBlue's default). Use 300 for files made for 300 dpi printers. It is ignored for `.labely`
  files, with a warning. This changes the command schema, so the schema hash changed. The API
  version stays 3, because nothing has been released since it was bumped.
- **Warnings.** The first warning names the dpi used. Each object or setting that does not come
  across gets its own warning.
- **Label.**
  - The size is the canvas size in dots ÷ dots per mm.
  - `rounded_rect` gets a 10-dot corner radius (NiimBlue's). `circle` gets half the short side.
  - `printDirection: "left"` (the default) prints the canvas turned 90° clockwise. The label is
    then turned clockwise with its content (`Document::rotate_label`), because Labely's width
    is the width across the printhead.
  - `split`, the cable tail, `mirror` and the print `offset` are dropped.
- **Placement.** Fabric places an object's origin point (`originX/Y`) at `left/top` and turns
  the box about it. The size is `width × scaleX` (`height × scaleY`). For shapes, the stroke
  width is added the way Fabric does (before scaling, or after it with `strokeUniform`). The
  importer converts this to Labely's frame, which turns about its centre. `flipX/Y` become
  `flip_h/v`, and `visible: false` becomes `hidden`.
- **Objects.**

  | NiimBlue | Labely |
  |---|---|
  | `Textbox` | `text`, wrapping |
  | `Text`, `IText` | `text`, no wrapping |
  | `QRCode` | `code2d` qr. `ecl` becomes `ecc`. `mode` and `qrVersion` are dropped |
  | `Barcode` CODE128, A, B, C, CODE39, CODE93, EAN13, EAN8, UPC, UPCE, ITF, ITF14, codabar | `barcode`. `printText` becomes `human_readable` below or none |
  | `Barcode` EAN5, EAN2, MSI*, pharmacode | skipped |
  | `Rect` | `rect`. `rx` becomes the corner radius |
  | `Circle`, `Ellipse` | `ellipse` |
  | `Polyline`, legacy `Line` | one `line` per segment |
  | `Image` with a data URL | `image`: an embedded asset, `fit: stretch` |
  | `ArUcoMarker`, `Group`, `Path`, others | skipped |

  Text details:
  - The font size in dots × `scaleY` becomes points.
  - `fontWeight` of bold or ≥ 600 becomes `bold`. `fontStyle` italic and `underline` map
    directly.
  - `textAlign` maps to `align`, `lineHeight` to `line_spacing`, and `charSpacing` (1/1000 em)
    to `letter_spacing_mm`.
  - `fontAutoSize` becomes `fit: shrink`.
  - "X Variable" font families lose the suffix.
  - Per-character `styles` are dropped.

  Shapes and images:
  - A fill or stroke counts only when it would print: it must be present, not transparent, and
    dark.
  - Image filters and crop are dropped.
- **Bindings.**
  - `{column}` becomes `{{column}}`.
  - `{dt±Nu|FORMAT}` becomes `{{now±Nu:FORMAT}}`. The units are the same, and Day.js tokens
    are translated to strftime.
  - `{$times}` stays as text.
  - `csv.data` becomes the document's data source. The first row is the header, and `\n` in a
    cell becomes a newline. A `$times` column is kept as data and does not repeat records.
- **Lists.** An array export opens its first label and warns how many there were.

## Consequences
- NiimBlue users can open their saved labels from the studio (the Open dialog also lists
  `.json`), the CLI (`labely run label.json`) and MCP (`document_open`).
- Text metrics differ between Fabric and Labely, so text that filled its box exactly can
  overflow a little. `doc.check` reports it.
- The fixtures in `crates/core/tests/fixtures/niimblue/` are written from the source schema,
  not exported from NiimBlue. A real export should be added when one is available.
