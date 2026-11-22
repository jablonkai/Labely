# MCP tools and commands

## Tools

| Tool | Runs | Notes |
|---|---|---|
| `command_list` | `command.list {prefix?}` | Every command with its JSON Schemas. Start here. |
| `command_run` | `{id, params}` | Runs any command. |
| `command_batch` | `command.batch` | Several steps, atomic by default, with `$N` references. |
| `document_new` | `doc.new` | |
| `document_open` | `doc.open {path, dpi?}` | The path must be under the allowed roots. A NiimBlue JSON export is imported (ADR-0043). |
| `document_save` | `doc.save {path?}` | The path must be under the allowed roots. |
| `document_get` | `doc.get` | The full document JSON. |
| `render_preview` | `render.preview` | Returns a PNG `image` block. `png_base64` is removed from the JSON. |
| `check` | `doc.check` | |
| `device_list` | `device.list` | Connected printers. |
| `device_status` | `device.status` | Battery, lid, paper (RFID). |
| `print` | `print.submit` | Returns `forbidden` until the user allows printing. |
| `undo`, `redo` | `history.undo`, `history.redo` | |

Hints:
- `readOnlyHint` is set for queries.
- `destructiveHint` and `openWorldHint` are set for device tools.

## Commands (API version 3)

| Id | Kind | Params |
|---|---|---|
| `command.list` | query | prefix |
| `command.batch` | edit | steps, atomic, stop_on_error, label, terse |
| `doc.new` | edit | width_mm, height_mm, name, corner_radius_mm, media, printer_model, density |
| `doc.open` | edit | path, dpi (NiimBlue import) |
| `doc.save` | query | path, doc_id |
| `doc.close` | edit | doc_id |
| `doc.list` | query | → docs (doc_id, name, path, dirty, active) in open order |
| `doc.activate` | edit | doc_id (not undoable) |
| `doc.get` | query | doc_id |
| `doc.set_label` | edit | name, width_mm, height_mm, corner_radius_mm, media, printer_model, density |
| `doc.rotate_label` | edit | direction (cw/ccw), rotate_content (default true) |
| `doc.check` | query | dpi, margin_mm, address, doc_id |
| `paper.list` | query | query (id, name, `50x30`, barcode), model, media, custom → papers [{id, name, width_mm, height_mm, corner_radius_mm, media, custom, barcodes}] |
| `paper.apply` | edit | id → label |
| `paper.add` | edit | name, width_mm, height_mm, corner_radius_mm, media, barcode → paper with its `my:` id |
| `paper.delete` | edit | id (`my:` only) → unlinked |
| `paper.link`, `paper.unlink` | edit | barcode, id → previous |
| `template.list` | query | query, folder → library, folders, templates [{ref, name, folder, starter, width_mm, height_mm, printer_model}] |
| `template.new` | edit | ref (`starter:<id>` or `my:[<folder>/]<name>`), name |
| `template.save` | edit | name, folder, overwrite, doc_id → ref |
| `template.move` | edit | ref, name, folder → ref |
| `template.duplicate` | edit | ref (a starter or library template), name, folder → ref |
| `template.delete` | edit | ref (library only) |
| `template.folder` | edit | op (create, rename, delete), folder, to, with_templates → folders |
| `export.png` | query | path, dpi, page, record |
| `export.pdf` | query | path, dpi, records or count → pages (one per label) |
| `element.add_text` | edit | frame\*, text, font_family, font_size_pt, bold, italic, underline, align, valign, line_spacing, letter_spacing_mm, wrap, fit (none, shrink, fill), invert, direction (horizontal, vertical), max_length_mm |
| `element.add_barcode` | edit | frame\*, data, symbology, human_readable, text_size_pt |
| `element.add_code2d` | edit | frame\*, data, symbology, ecc |
| `element.add_image` | edit | frame\*, path or data_base64, mime, fit, dither, threshold, brightness, contrast, invert |
| `element.add_rect` | edit | frame\*, stroke_mm, fill, corner_radius_mm |
| `element.add_ellipse` | edit | frame\*, stroke_mm, fill |
| `element.add_line` | edit | frame\*, stroke_mm, dash_mm |
| `element.add_table` | edit | frame\*, row_count, column_count or cells (row-major texts), stroke_mm, font_size_pt, font_family, align |
| `element.table_edit` | edit | id, edit: {op: insert_row, delete_row, insert_column, delete_column, set_cell, merge, split, resize_row, resize_column, clear} |
| `element.add_icon` | edit | frame\*, icon (from icon.list), keep_aspect, invert |
| `element.add_border` | edit | frame\*, style (single, double, thick_thin, dashed, dotted, corners, ticket, scallop), stroke_mm, corner_radius_mm |
| `icon.list` | query | category |
| `font.list` | query | → fonts (family names for font_family) |
| `font.add` | ui | path (TTF/OTF/TTC) → added, fonts; loads the file for this session |
| `code.decode` | query | path or data_base64 → codes [{kind, symbology, text}] |
| `element.set_props` | edit | id, props (any frame or kind property, plus flip_h, flip_v; text `spans` [{start, end, bold, italic, underline, font_size_pt}]; image `crop` {left, top, right, bottom} in 0–1; `mirror: null` unlinks a mirror copy) |
| `element.move` / `.resize` / `.rotate` | edit | id, x_mm, y_mm / width_mm, height_mm / rotation_deg |
| `element.delete` | edit | ids |
| `element.duplicate` | edit | id, dx_mm, dy_mm |
| `element.mirror` | edit | id, axis (horizontal = left–right, vertical = top–bottom) → id of a linked copy that follows the source |
| `element.paste` | edit | elements (as in doc.get), assets, dx_mm, dy_mm → ids |
| `element.reorder` | edit | id, to (front, back, forward, backward) |
| `element.align` | edit | ids, edge (left, center, right, top, middle, bottom), to (selection, label; default selection for 2+ ids) → moved |
| `element.distribute` | edit | ids (3+), axis (horizontal, vertical) → moved |
| `element.group` | edit | ids (2+; members bring their whole group) → group, members |
| `element.ungroup` | edit | ids → ungrouped |
| `element.lock` | edit | ids, locked (default true) |
| `history.undo`, `history.redo` | edit | doc_id |
| `history.journal` | query | since |
| `selection.set` | ui | ids, mode (replace, add, remove, toggle) |
| `selection.get` | query | doc_id |
| `variable.set` | edit | name, value (used as `{{name}}`) |
| `data.attach_csv` | edit | path or text, delimiter, header |
| `data.attach_xlsx` | edit | path (.xlsx, .xls, .ods), sheet, header |
| `data.attach_images` | edit | paths (1–500), column (default `image`), place, fit, dither → one record per image, element_id |
| `data.detach` | edit | doc_id |
| `data.get` | query | from (1-based), limit |
| `data.bind` | edit | id, column (a column, serial or variable) |
| `data.preview_record` | query | record (1-based) |
| `data.find_record` | query | value (trimmed, exact), column (default: any) → first record (1-based), matches |
| `data.set_serial` | edit | name (default `serial`), start, step, digits, prefix, suffix |
| `data.remove_serial` | edit | name |
| `render.preview` | query | dpi, page, record |
| `device.scan` | device | timeout_ms |
| `device.list` | device | |
| `device.models` | device | |
| `device.connect`, `.disconnect`, `.status` | device | address |
| `paper.detect` | device | address, doc_id → rfid, barcode, media, remaining, paper, matches_label |
| `device.calibrate` | device | address, media (default: the label's) |
| `device.settings` | device | address → settings {auto_off_level, sound_power, sound_bluetooth} |
| `device.set_settings` | device | address, auto_off_level (1–4), sound_power, sound_bluetooth → settings |
| `print.submit` | device | address, copies, density, records, count, copies_column, offset_x_mm, offset_y_mm, mirror, rotate_180, grayscale (models with `gray`), wait → job_id, state |
| `job.list` | query | → paused, jobs [{id, state, address, doc_name, pages_done, pages_total, error, …}] |
| `job.get` | query | id |
| `job.wait` | device | id, timeout_ms (default 60000, max 600000) |
| `job.cancel` | device | id |
| `job.pause`, `job.resume` | device | → paused |
| `job.clear` | device | → removed (finished jobs only) |
| `job.history` | query | limit, offset → total, entries [{id, printed_at, doc_name, pages_printed, state, file}] |
| `job.history_open` | edit | id → doc_id, name |
| `job.history_delete` | edit | ids |
| `job.history_clear` | edit | |

\* `frame` is `x_mm, y_mm, width_mm, height_mm`, plus optional `rotation_deg` and `name`. Every
document command also takes an optional `doc_id`; the default is the active document. The JSON
Schemas from `command_list` are the authority.

Template library writes (`template.save`, `.move`, `.duplicate`, `.delete`, `.folder`) are not
journaled and cannot be undone. They address templates by reference inside the library
directory, so they are not limited by the file roots; `export.*` paths are.

Print jobs go through a queue with one worker. `print.submit` waits for its job by default;
with `wait: false` it returns `{job_id, state: "queued"}` at once. A paused queue finishes the
running job and starts no new one; a waited `print.submit` on a paused queue fails with
`bad_params`. `job.cancel` drops a queued job or stops the running one at the next label
(state `cancelled`, error code `cancelled`). In the studio an agent's `job.wait` waits at most
2 s per call, so poll it. `job.*` commands are not gated by "Allow agents to print". The print
history (`job.history*`) keeps every job that reached a printer, with a snapshot of the
document; it is not limited by the file roots, and its writes cannot be undone.

Label paper: `std:` papers are Labely's bundled sizes, `my:` papers are the user's. The printer
reads a barcode from the roll's RFID tag; `paper.link` remembers which paper that barcode is,
so `paper.detect` and the `paper_mismatch` check recognise the roll next time. To use the
loaded paper: `paper.detect`, then `paper.apply {id}` when `matches_label` is false; for an
unknown barcode, `paper.link {barcode, id}` (or `paper.add {…, barcode}`) first. Paper library
writes are not undoable or journaled and are not limited by the file roots.
`device.calibrate` feeds labels, so like `print.*` it needs "Allow agents to print".
Settings a printer does not report are left out of `device.settings`.

Printer addresses:
- `ble:<id>`
- `usb:<port>`
- `tcp:<host>[:<port>]`: a network printer, port 9100 by default. An IPv6 address goes in
  brackets when a port follows it: `tcp:[fe80::1]:9100`.
- `ble:`, `usb:` and `tcp:` take `?model=<id>` for a printer that is not a NIIMBOT and cannot
  say what it is, for example `tcp:192.168.1.50?model=generic.zpl_203`. `device.models` lists
  the ids.
- `sim:<model id>`: the protocol simulator.
- `fake:<dir>[?model=<id>&delay_ms=<n>]`: writes PNGs to `<dir>/job-NNNN/`; `delay_ms` makes
  each page take that long, to watch the queue.

## Bindings

`{{name}}` in a text, barcode or 2D-code string is filled at render and print time. It resolves
to a data column, then a serial, then a variable. Date bindings are computed at render time:

| Binding | Example output |
|---|---|
| `{{now}}` | `2026-11-14` |
| `{{now:%d.%m.%Y %H:%M}}` | `14.11.2026 09:30` (chrono strftime) |
| `{{now+30d}}`, `{{now-2h:%H:%M}}`, `{{now+1M:%Y-%m}}` | offset units: s, m (minutes), h, d, w, M (months), y |

An image element whose `asset` is a binding, e.g. `{{image}}`, shows the asset whose key the
binding yields. `data.attach_images` fills such a column with one asset key per record.

The clock is the local time unless the `now` variable is set (`YYYY-MM-DD[ HH:MM[:SS]]`), which
pins it. Pin it for reproducible previews and tests.

## `doc.check` rules

| Rule | Severity | Meaning |
|---|---|---|
| `out_of_bounds` | warning | The element extends outside the label. |
| `in_margin` | warning | The element is inside `margin_mm` of the edge. Checked only when `margin_mm` is given. |
| `outside_printable` | warning | The element is outside the model's printable head width. |
| `label_too_wide` | error | The label is wider than the model's print width. |
| `dpi_mismatch` | warning | The check dpi differs from the model's dpi. |
| `density_out_of_range` | error | The density is outside the model's range. |
| `paper_mismatch` | warning | The printer's RFID paper differs from the label: size and media when its barcode is linked to a paper, else media only. The hint names the `paper.apply` id. |
| `binding_invalid` | error | A date binding has a bad offset or format, e.g. `{{now+3q}}`. |
| `binding_missing` | warning | A `{{name}}` matches no data column, serial or variable. |
| `binding_empty` | info | A bound variable is empty, or a bound column is empty in some records. |
| `data_empty` | warning | The attached data source has no records. |
| `render_failed` | error | The page could not be rendered. |
| `text_overflow` | error | The text does not fit its box. |
| `text_shrunk` | info, or warning below 5 pt | The text was auto-shrunk. |
| `encode_failed` | error | The data is invalid for the symbology. |
| `module_too_narrow` | warning | The narrowest bar or module is under 2 dots. |
| `quiet_zone` | warning | There is not enough blank space around the code: 10 modules for 1D, 4 for QR. |
| `code_unreadable` | warning | The code does not decode back from the rendered raster. |
| `bad_asset` | error | The image is missing or undecodable, the icon is unknown, or the table is inconsistent. |
| `font_fallback` | info | The requested font is unavailable. |
| `low_contrast` | warning | The dithered image is under 2 % or over 98 % black. |

Paper size is not compared yet, because that needs the label paper database (Phase 3).
