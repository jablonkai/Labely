# ADR-0016: Tables, icons, borders and reading codes from images

- Status: proposed
- Date: 2026-11-14

## Context
The feature matrix lists a table element, an icon/sticker library, nine-patch borders, and
decoding a QR code from an uploaded image (all Ph3). The vendor's icon and border libraries
are cloud content and VIP-gated; Labely ships its own.

## Decision
- **Table element.** `ElementKind::Table { column_widths, row_heights, cells, merges,
  stroke_mm, font_family, font_size_pt, align }`. Sizes are relative weights, so the grid
  always fills the frame and resizing the element scales it. Cells are row-major
  `TableCell { text, bold, align, font_size_pt, invert }`. Text shrinks to fit its cell and
  takes bindings. Merges are non-overlapping `CellRange`s, and grid lines inside a merge are
  not drawn. At most 64×64.
- **Editing.** `element.add_table` creates a table, and `element.table_edit {id, edit: {op…}}`
  performs one structural edit (rows, columns, cells, merges, sizes). Every edit is one undo
  step and keeps merges consistent: inserting inside a merge widens it, and deleting shrinks
  it until it is dropped. `element.set_props` can still replace the whole table and is checked
  by `TableProps::validate`.
- **Icons.** `ElementKind::Icon { icon, keep_aspect, invert }` names an icon from a bundled set
  of original 24×24 SVGs in `labely-render` (handling, safety, symbols, office, food). They
  render with resvg 0.50 (no default features, same tiny-skia 0.12). `Renderer::icons()` and
  `icon.list` expose the catalogue. An unknown name is a `bad_asset` check error.
- **Borders.** `ElementKind::Border { style, stroke_mm, corner_radius_mm }`. The styles are
  drawn procedurally: single, double, thick_thin, dashed, dotted, corners, ticket, scallop.
  This replaces nine-patch images: they stay crisp at any size and dpi and need no assets.
- **Reading codes.** `Renderer::decode_image(bytes)` decodes an image file with rxing (images
  are scaled to at most 2400 px, plus a white margin). `code.decode` returns
  `{kind, symbology, text}`, which feed straight into `add_barcode`/`add_code2d`. The studio's
  Insert → Code from image does both steps.
- **Format.** New kinds and fields in the unreleased v2; no bump.

## Consequences
- User-supplied SVG icons and borders (custom uploads) are not supported yet. SVG image assets
  could reuse resvg later.
- Per-cell borders and cell padding are not configurable.
