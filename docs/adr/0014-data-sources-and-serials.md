# ADR-0014: Data sources and serial numbers

- Status: proposed
- Date: 2026-11-14

## Context
Phase 3 adds batch printing from CSV and spreadsheets, and serial numbers. Architecture §3.1
sketched `data_sources: Vec<DataSource>` and serial elements. The vendor app binds one data
source per template, and its serial element is text with counter settings.

## Decision
- **One embedded source.** `Document.data: Option<DataSource { origin, columns, rows }>`. The
  cells are embedded as text, so a `.labely` file prints the same anywhere and undo covers the
  data. Limits: 10 000 records, 256 columns, 32 MiB input files.
- **Readers.** `csv` for CSV (delimiter sniffed from `, ; tab |`, BOM stripped, UTF-8 only) and
  `calamine` for xlsx/xlsm/xls/ods. Dates become `YYYY-MM-DD[ HH:MM[:SS]]`, and whole numbers
  lose the `.0`. Both go through `Ctx::check_path` (ADR-0012).
- **Serials are named counters, not an element kind.** `Document.serials: name → SerialSpec
  { start, step, digits, prefix, suffix }`. Any string property can use `{{name}}`, so a text,
  a barcode and a QR code can share one counter. The studio's "serial" insert is a text element
  bound to a counter.
- **Resolution.** For the label at job position `n` showing row `r`, the values are the
  variables, then the serials at `n`, then the columns of row `r`. A column wins over a serial
  or a variable with the same name, and the attach commands warn about the clash.
- **Records are 1-based** in every param (`record`, `records: "1,3,5-7"`), because users count
  rows that way. Pages stay 0-based.
- **Copies.** `copies` stays job-wide and maps to the printer's quantity. `copies_column`
  expands pages instead and sends quantity 1.
- **Format v2.** `FORMAT_VERSION` is 2. v1 files load unchanged (the new fields are optional)
  and are saved as v2.

## Consequences
- Large spreadsheets make large `.labely` files. A linked (not embedded) source can be added
  later as `DataSource.link` without breaking v2.
- Jobs render every label up front (at most 5000). The lazy per-record queue in architecture §5
  comes with the print queue later in Phase 3.
