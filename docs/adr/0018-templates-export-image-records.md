# ADR-0018: Templates, export and image records

- Status: proposed
- Date: 2026-11-15

## Context
Slice 3.4 of the feature matrix: an own starter set instead of the vendor's cloud gallery, a
local template library with folders, PNG and PDF export, and printing a batch of images, one
label each. The vendor app keeps templates in a cloud account and caps folders and bulk image
import for non-members; Labely keeps everything local and unlimited.

## Decision
- **Starters.** Ten original templates (`crates/core/templates/*.labely`) are compiled into
  `labely-core`. They are built with commands and must pass `doc.check` without errors (a
  test checks this). The studio shows localised names from `template-<id>` keys.
- **Library.** A directory set by the frontend (`Session::set_template_dir`); the CLI, the
  headless MCP server and the studio use `<app-data>/templates`. Templates are `.labely`
  files in it (the default folder) or in one level of sub-folders. There is no index file:
  the file system is the source of truth, so users may also copy files in by hand.
- **References, not paths.** Commands take `starter:<id>` or `my:[<folder>/]<name>`. Names
  are 1–80 characters, may not start with `.`, and may not contain `/ \ : * ? " < > |`, so a
  reference can never leave the library. For that reason library commands do not go through
  `Ctx::check_path`: agents may manage templates even when their file roots do not include
  the library. `export.*` writes arbitrary paths and does call `check_path`.
- **Commands.** `template.list`, `.new`, `.save`, `.move` (rename and/or move), `.duplicate`
  (also copies a starter into the library), `.delete` and `.folder {op: create | rename |
  delete}`. Library writes are `edit` commands with no undo and no journal entry: they change
  files, not documents, and replaying a journal should not touch the library. Deleting a
  non-empty folder needs `with_templates: true`.
- **Export.** `export.png` writes one label (page, record). `export.pdf` writes one PDF page
  per label (records × pages, or `count` without data), each page the size of the label in
  mm. The pages are the exact 1-bit print rasters, so the PDF shows what the printer burns.
  The PDF writer is ~100 lines in `labely-core` (one `DeviceGray` 1-bit image per page,
  RunLengthDecode), so no PDF dependency is added.
- **Image records.** `data.attach_images {paths}` embeds the images as assets and attaches a
  data source with columns `image` (asset key) and `file` (file name), one record per image.
  An image element whose `asset` is `{{image}}` is resolved per record by the renderer, and
  one filling the label is placed unless it exists. Batch printing and PDF export then work
  through the existing record machinery, one label per image. Replacing or removing a data
  source prunes assets that nothing references any more.
- **Studio.** File → New from template… opens a gallery (starters, the default folder and
  sub-folders, search, thumbnails rendered from the print raster) with open, duplicate,
  rename, move, delete and folder actions. File → Save as template…, Export PNG…, Export
  PDF… (Ctrl+E) and Data → Attach images… run the commands above.

## Consequences
- No new format version: a bound image asset is just a string, and v2 readers that do not
  resolve it report `bad_asset`.
- Not done in this slice, with reasons:
  - PDF import (split a PDF into page images) needs a PDF rasteriser; no small, permissive,
    pure-Rust one fits yet. Images exported from a PDF viewer can be attached instead.
  - The read-only importer for the vendor's template JSON waits for sample files; the format
    is only known from symbol names (feature matrix, UNCERTAIN).
  - Local font files (TTF/OTF) are left for a later slice.
  - C1 tube templates come with C1 support in Phase 6.
