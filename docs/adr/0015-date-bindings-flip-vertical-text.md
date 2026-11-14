# ADR-0015: Date bindings, flip and vertical text

- Status: proposed
- Date: 2026-11-14

## Context
The feature matrix asks for a date/time element (formats, current time, offsets, refresh at
print), flip/mirror and vertical text. ADR-0008 already reserves `{{now:fmt}}` as a binding.

## Decision
- **Dates are bindings, not an element kind.** `{{now[±N unit][:strftime]}}` works in any text,
  barcode or 2D-code string, alongside column and serial bindings (ADR-0014). Units: s, m, h, d,
  w, M, y; months clamp to the month end. The default format is `%Y-%m-%d`. The studio's
  Insert → Date and Insert → Time add a text element with a date binding.
- **Clock.** Local time from `chrono` (`clock` feature) at render time, so every print shows the
  current time. The `now` variable pins the clock for previews and tests.
- **Validation.** An invalid offset or format is left as literal text when rendering, and
  `doc.check` reports it as `binding_invalid` (error).
- **Flip.** `Element.flip_h` and `flip_v` mirror the content inside the frame before rotation.
  They apply to every kind, are set with `element.set_props`, and are omitted from JSON when
  false.
- **Vertical text.** `TextProps.direction: horizontal | vertical`. Vertical text stacks
  characters one per row (source lines are separated by a blank row) through the normal text
  layout, so fit and shrink still work. Text turned 90° is `rotation_deg`; curved text is
  Phase 6.
- **Format.** These are optional fields that serialise only when set. Format v2 is not released
  yet, so they join v2 with no version bump.

## Consequences
- A "related time" linked to another element (vendor `associateId`) is not modelled. Two date
  bindings with different offsets cover the common best-before case.
- Vertical CJK punctuation is not rotated or repositioned.
