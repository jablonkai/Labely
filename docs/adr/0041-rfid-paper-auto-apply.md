# ADR-0041: Automatic use of a recognised paper, and a bundled RFID table

- Status: proposed
- Date: 2026-11-21

## Context
A roll's RFID tag has a barcode but no size (protocol.md §10). ADR-0020 made a roll known by
linking its barcode to a paper once by hand, and the studio offers "Use" when the loaded paper
differs from the label. It left out applying the paper without asking. The roadmap parked two
follow-ups that need no new hardware: a setting that applies a recognised paper, and a bundled
barcode table so that common rolls are known on first load.

## Decision
- **Setting.** `auto_apply_paper` in the studio preferences (`studio.json`), off by default.
  The switch is in the Print menu and in the printers panel: "Use the detected paper
  automatically".
- **When it applies.** Only where a roll is read on purpose: on connect, and from
  Print → Detect paper. With the setting on, a recognised paper that differs from the label is
  applied with `paper.apply`, the status bar names the paper, and the view is fitted. It is
  one undo step. The paper library and the new-document dialog still only detect.
- **It stays in the studio.** `paper.detect` still never changes the document (ADR-0020), so
  an agent or the CLI sees no change. The mobile shells show no RFID paper yet.
- **Bundled table.** `crates/core/papers/builtin.json` gets a top-level `rfid` map from
  barcode to bundled paper id. `UserPapers::by_barcode` looks at the user's link first and then
  at the table, and `barcodes_of` lists both. A bundled entry cannot be unlinked;
  `paper.unlink` says to override it with `paper.link`.
- **The table is empty.** It holds only barcodes read from rolls we own, not vendor data
  (ADR-0001), and the B21 Pro did not answer on BLE during this slice. The roll in it is added
  when it is read. A test checks that every entry points at a bundled paper.

## Consequences
- No schema or API version change: `Paper` and the command schemas are as they were.
- Measuring the label with 0x59/0x5A stays under Later. It needs the printer.
