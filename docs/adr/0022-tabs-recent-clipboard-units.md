# ADR-0022: Document tabs, recent files, clipboard, rulers and units

- Status: proposed
- Date: 2026-11-15

## Context
Slice 3.6b of the Phase 3 exit audit covers the Ph2/Ph3 rows of the feature matrix for the
studio shell:
- the tabbed main window, recently used files, rulers and the printable-area overlay;
- rotating the label paper, copy and paste, and inch units.

The core session could already hold several documents, but no command switched between them,
and the studio kept no state between runs.

## Decision
- **Core commands.**
  - `doc.list` (query): open documents in open order.
  - `doc.activate` (journaled, not undoable, like `doc.close`).
  - `doc.rotate_label` (undoable): a quarter turn. Width and height swap. With
    `rotate_content` every element turns about the label: its centre moves and 90° is
    added to its rotation.
  - `element.paste` (undoable): inserts element JSON as `doc.get` returns it, with fresh
    ids. Groups among the pasted elements become new groups, and a group of one is dropped.
    Locks are cleared. Image assets come from the document or from `assets`, whose keys must
    be the sha256 of the data.
- **Tabs.** One tab per open document, with a dirty mark. Click switches, × or a middle click
  closes, and + opens New label. Closing a document with unsaved changes asks Save / Don't
  save / Cancel. A new empty document closes without asking.
- **Preferences.** `<app-data>/studio.json` (`StudioOptions::prefs_file`) holds up to 10 recent
  files and the length unit. It is written through a temp file and a rename. A missing or
  invalid file gives the defaults. Files the user opens or saves join the recent list; files
  opened by agents do not. A recent file that is gone is removed when picked.
- **Clipboard.** Cmd+C/X/V arrive in egui as clipboard events. Copy writes
  `{"format": "labely/elements", "elements", "assets"}` to the system clipboard, so it works
  across studio windows. Pasting this text into the same document offsets the copy by 1 mm,
  and each further paste moves on by 1 mm. In another document it lands in place. Any other
  text becomes a text element. Edit → Paste uses the last studio copy, because egui cannot
  read the system clipboard on demand.
- **Rulers and overlay.** Rulers along the top and left edges are zeroed at the label corner.
  Tick spacing adapts to the zoom: minor ticks at least 5 px apart, labels at least 40 px.
  When the model's head (`head_dots / dpi`) is narrower than the label, the parts that cannot
  print are shaded. This uses the same rule as `doc.check outside_printable`.
- **Units.** mm or inches (View → Units). Documents and commands stay in mm. The studio only
  formats lengths (`_mm` fields, the New label, paper and print-offset dialogs, the rulers) and
  parses them. A typed `mm`, `in` or `"` suffix overrides the chosen unit.

## Consequences
- API version stays 3, because there has been no release since it was bumped. The schema hash
  changed.
- Not done: tab reordering, a thumbnail list of recent files, and native clipboard formats
  other than text.
