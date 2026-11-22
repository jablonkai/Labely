# ADR-0044: ZPL import

- Status: proposed
- Date: 2026-11-22

## Context
Labely prints ZPL (ADR-0037), but it could not open a ZPL label (issue #46). NiimBlue imports
ZPL as a picture through the Labelary web API. That sends the label to a third party, so it
was rejected. A small, offline importer can turn the common commands into editable Labely
elements instead. The command semantics come from Zebra's public ZPL II programming guide. No
code was copied (ADR-0001).

## Decision
- **Read-only, through `doc.open`.** The content decides the format. A file that does not
  start with `{` or `[` and holds `^XA` is ZPL. The extension does not matter. Like a NiimBlue
  import, it opens with no path and unsaved. There is no export back to ZPL source; printing
  ZPL stays the raster path of ADR-0037.
- **`dpi`.** The same optional `doc.open` parameter as NiimBlue (100–1200, default 203). It
  converts dots to mm. This changed only the command descriptions, so the schema hash stayed
  the same and only the snapshot changed.
- **Scope.** Only the first `^XA … ^XZ` format is read. A warning names how many there were.
  Commands are read case-insensitively. CR and LF are ignored, as the printer ignores them.
- **Label.**
  - `^PW` sets the width and `^LL` the length. When one is missing, the content's extent is
    used, with a warning.
  - `^LH` offsets every field. `^MN` Y/W, M and N set the media to gap, black mark and
    continuous.
- **Fields.** `^FO` places the top-left of the field's bounding box. `^FT` places the
  bottom-left of the turned field (an approximation of the baseline). `^FW`, or the field's
  own orientation, maps N/R/I/B to 0/90/180/270°.

  | ZPL | Labely |
  |---|---|
  | `^FD`/`^FV` text with `^A` or `^CF` | `text`. The cell height is one line (font size = height / 1.2). Font `0` is bold |
  | … with `^FB` | wrapping text box: the `^FB` width, line count × (height + spacing), alignment. `\&` is a new line |
  | … without `^FB` | one line, `fit: shrink`. The width is estimated: the cell width per character for bitmap fonts `A`–`H`, 0.6 × the width for scalable ones |
  | `^FR` on text | `invert` |
  | `^BC` | `barcode` Code 128. A leading `>;`, `>:`, `>9` forces set C, B, A. Mode D is GS1-128 |
  | `^B3`, `^BE`, `^BU` | `barcode` Code 39, EAN-13, UPC-A |
  | `^BQ` | `code2d` QR. ECC comes from the `^FD` prefix (`QA,`), and the manual-mode prefix is removed. The size is the smallest version for the data × the magnification |
  | `^BX` | `code2d` Data Matrix, sized from the module size and the data |
  | `^GB` | `rect`. It is filled when the border reaches the middle (bars and lines). The rounding 0–8 becomes the corner radius |
  | `^GC`, `^GE` | `ellipse` |
  | `^GD` | `line` |
  | `^GF` A (hex, ZPL run-length compression, `:B64:`, `:Z64:`) and B | `image`: a 1-bit BMP asset, `fit: stretch`, threshold |

  - Barcode width comes from the module count × the `^BY` module width. Its height is the
    bar height plus an interpretation line of 9 × the module width.
  - `^FH` hex escapes are decoded. Data is UTF-8 when it is valid UTF-8 (always under
    `^CI28`). Otherwise it is read as Latin-1.
- **Warnings.**
  - Every other command gets a warning: "`^XG` is not supported and was skipped".
  - These are also reported:
    - `^FR` on graphics and barcodes, `^LRY`, `^POI`, a non-zero `^LS`/`^LT`;
    - white graphics;
    - `^GFC`;
    - non-dot `^MU`;
    - `^FN` (imported as its default data) and `^SN` (its start value);
    - Code 128 `>` codes after the first.
  - Settings that do not change the layout (`^FX`, `~SD`, `^MD`, `^PQ`, `^PR`, `^MM`, `^MT`,
    …) are ignored without a warning.

## Consequences
- ZPL labels open in the studio (the Open dialog lists `.zpl`), the CLI
  (`labely run label.zpl`) and MCP (`document_open`). No data leaves the machine.
- Text widths and code sizes are estimates. Text without `^FB` shrinks to fit its estimated
  box. `doc.check` reports overflow and codes that are too small.
- Stored formats and graphics (`^DF`, `^XF`, `~DG`, `^XG`), PDF417 and the other symbologies
  are not imported. They can be added one by one as fixtures turn up.
