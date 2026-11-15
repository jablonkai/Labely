# ADR-0021: Print offset, mirror and output direction; Code 128 sets and GS1-128

- Status: proposed
- Date: 2026-11-15

## Context
The Phase 3 exit audit (slice 3.6) found Ph1–Ph3 rows of the feature matrix that were still
open. Slice 3.6a covers the printing and barcode rows:
- print offsets (matrix §6), mirror print for transparent or iron-on media (§6), and the output
  direction (§2);
- the Code 128 code sets A, B and C and GS1-128 (§3, Symbologies).

## Decision
- **Job transforms, not document fields.** `print.submit` gains `offset_x_mm` and
  `offset_y_mm` (±10 mm), `mirror` and `rotate_180`. They describe how a particular printer
  and roll take the label, not the label itself, so they are not stored in `.labely` and the
  file format stays v2. Core applies them to every rendered page, after rendering and before
  the driver. The order is offset (in label coordinates), then the left-to-right mirror, then
  the 180° turn. Content moved past an edge is clipped. Preview and export are not changed.
- **Output direction.** The model's feed direction stays in the printer registry. The user
  setting is `rotate_180`, so labels leave the printer the other way round.
- **Frontends.** The CLI adds `--offset-x`, `--offset-y`, `--mirror` and `--rotate-180`. The
  studio print dialog adds offset fields and two checkboxes. It keeps them for the session.
- **Code 128 sets.** `Symbology1d` gains `code128_a`, `code128_b` and `code128_c`. They use
  rxing's `ForceCodeSet` hint. Data the set cannot encode gives an `encode_failed` issue.
- **GS1-128.** `gs1_128` takes AIs in brackets, e.g. `(01)09501101530003(17)250101`, or a raw
  element string with ASCII GS (0x1D) separators. Labely writes a leading FNC1, plus an FNC1
  after each variable-length field that another AI follows. Fixed-length AIs are checked
  against GS1's predefined-length table (00–04, 11–20, 31–36, 41). Other AIs are not checked
  against the full GS1 dictionary. The human-readable line shows the data as typed.

## Consequences
- API version stays 3, because there has been no release since it was bumped. The schema hash
  changed.
- Not done in 3.6a: a stored per-printer default offset, and GS1 check-digit validation.
