# ADR-0008: Open document format `.labely`

- Status: proposed
- Date: 2026-11-14

## Decision
- **Format.** `.labely` is UTF-8 JSON with `format_version` (integer) and forward migrations in
  core. Its JSON Schema is published in `docs/format/`.
- **Units.** mm, as f64.
- **Ids.** Stable `u64` element ids.
- **Bindings.** `{{column}}`, `{{serial}}` and `{{now:fmt}}` in string fields.
- **Assets.** Small assets are embedded as base64 with a sha256 key. Large ones go in `.labelyz`, a
  zip of `document.json` plus `assets/`.
- **Vendor templates.** Importing vendor template files is not planned. Users recreate templates,
  or a separate clean-room import spec is written first, as its own ADR.

## Consequences
- The format is diff-friendly and agent-friendly. The schema doubles as documentation for
  external tools.
