# ADR-0023: Fill-to-box text, length guard, fonts, rich spans, image crop and mirror copies

- Status: proposed
- Date: 2026-11-15

## Context
Slice 3.6c of the Phase 3 exit audit covers the remaining text and data rows of the feature
matrix: auto font size to fill the box, the max-length guard, a font picker, dragging a data
field onto an element or generating an element from a column, rich text, image crop and
mirror copies linked to the original.

## Decision
- **Fit.** `TextFit::Fill` picks the largest size, in 0.1 pt steps from 1 pt up to the box
  height (at most 1000 pt), at which the text fits. It grows or shrinks. `shrink` still only
  shrinks. Both report `text_shrunk` when the result is below the set size.
- **Length guard.** `TextProps::max_length_mm` is optional. When the longest laid-out line is
  wider, `doc.check` reports `text_too_long` (a warning). It is a check, not a clip, so it
  also catches unwrapped text that overflows a long box.
- **Fonts.** `Renderer::fonts()` lists family names. The bundled-only renderer lists Noto
  Sans; with system fonts it lists every installed family. The new `font.list` query exposes
  them to agents, and the studio's `font_family` field is a drop-down of them.
- **Rich text.** `TextProps::spans` holds up to 256 `{start, end, bold?, italic?,
  underline?, font_size_pt?}` ranges in characters over `text`. Unset fields inherit the
  element style, and later spans win. The renderer splits the text at span edges, substitutes
  bindings per piece and shapes it with cosmic-text rich text. A span's size scales with the
  element's size under `shrink` or `fill`. Spans past the end of the text are clamped rather
  than rejected, so changing `text` never fails. Vertical text ignores spans. In the studio,
  B / I / U / A− / A+ style the selection of the text field. Typing moves the spans with the
  characters (common prefix and suffix diff).
- **Crop.** `ImageProps::crop` `{left, top, right, bottom}` holds fractions 0–1 of the source
  image. It is applied before `fit`. An invalid crop is rejected by commands and ignored by
  the renderer. The studio shows four percentage sliders and a reset.
- **Mirror copies.** `Element::mirror: {of, axis}` marks a linked copy. After every
  successful edit command, the session re-derives each copy from its source:
  - kind, size, hidden state and rotation (negated);
  - its position reflected across the label's vertical (`horizontal`) or horizontal
    (`vertical`) centre line, with the matching flip toggled.

  This runs inside the same undo step. `element.mirror` creates a copy. `mirror: null` via
  `element.set_props` unlinks it. A link to a missing source, to another page or to another
  copy is dropped and the copy stays as it is. Duplicating a copy gives a plain element.
  Pasting keeps a link only when its source is pasted along.
- **Columns.** In the studio's data panel, column names can be dragged. Dropping one on a
  text, barcode or 2D code calls `data.bind`, and dropping it on empty canvas adds a text
  element bound to it at the pointer. The "+" menu beside each column adds a text, barcode or
  2D code bound to it. No new core command is needed (`data.bind`, `element.add_*`).

## Consequences
- API version stays 3, because there has been no release since it was bumped. The schema hash
  changed. The `.labely` format gains optional fields only, so `FORMAT_VERSION` stays 2:
  older files load unchanged.
- Edits made directly to a mirror copy's content are overwritten by the next sync. The
  studio shows the link with an Unlink button.
- Not done: an in-place WYSIWYG rich-text editor on the canvas, per-span font families, and
  interactive crop handles on the canvas.
