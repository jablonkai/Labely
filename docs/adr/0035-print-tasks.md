# ADR-0035: NIIMBOT print tasks (Phase 6 start)

- Status: proposed
- Date: 2026-11-20

## Context
Phase 6 adds printers and packaging (roadmap). The roadmap says a new model is a registry entry
plus protocol tests. A check of the registry against the driver showed that this did not hold
yet: the registry has 57 NIIMBOT models, and the driver knew three of the seven print tasks of
protocol.md §5.2. The B21, B21-L2B, D11 and H1S were listed as supported and failed on print
with "not implemented yet". Only the B21 Pro job was tested. Two models, D11 and Hi-NB-D11,
share model code 512 with different heads and tasks, and the first one always won.

The three Phase 5 hardware checks (ADR-0034) are still open. They do not block this slice.

## Decision
- **All seven tasks.** `PrintTask` gains `D11V1`, `B21V1`, `B21L2b` and `H1s`. The sequences
  follow niimbluelib (MIT), as protocol.md §5.2 records them:
  - *D11_V1:* 1-byte PrintStart, PrintClear, PageStart, 2-byte size, SetQuantity. The printer
    reports each page with an unsolicited PageIndex (0xE0).
  - *B21_V1:* 1-byte PrintStart. The printer takes no copy count, so each copy is sent as a
    page with a 4-byte size. PrintEnd is polled every 500 ms until it answers true (5 minutes
    at most).
  - *B21_L2B:* as B21_V1, but PageStart (5 tries) and PageEnd (20 tries) are retried every
    500 ms, and nothing is polled.
  - *H1S:* 2-byte PrintStart with the page total, PageStart, 9-byte size. Cut height and cut
    type stay zero until the cutter slice.
- **Check-lines.** The two B21 tasks send a check-line (0x86) after every 200th row and wait
  for 0xD3. No row run crosses a check-line. They use the total count mode. `Step::Sync` waits
  for the reply and ignores its payload, because the payload is not documented.
- **Page index.** `Step::WaitPageIndex` waits for 0xE0 until the page count is reached. The
  driver also records a 0xE0 that arrives while it waits for another reply, so an early report
  is not lost.
- **Pages printed.** A task with no wait step counts the answered PageEnds.
- **Task names in one place.** `PrintTask::from_name` maps the registry `task` strings. The
  driver only decides `auto` (protocol 4 and newer use D110M_V4, older use B1).
- **Shared model codes.** When the model guessed from the BLE name has the reported code, it
  wins. Otherwise the first registry entry for the code is used, as before.
- **Every model is tested.** `SimConfig::for_model` sets the simulator up for a registry model.
  One test prints two copies on each of the 57 models and compares the pages the simulator
  received with the prepared raster. The simulator now answers check-lines, sends PageIndex,
  takes the copy count from SetQuantity and the width from the head for the 2-byte size.

## Phase 6 slices

| Slice | Scope |
|---|---|
| 6.1 Print tasks | the four missing NIIMBOT tasks; every registered model prints on the simulator; shared model codes (this ADR) |
| 6.2 NIIMBOT variants | two-colour and grayscale rows, cutter and half-cut, the OSS-only models |
| 6.3 Other vendors | Phomemo, Marklife, generic ESC/POS, TSPL and ZPL drivers |
| 6.4 Desktop packaging | cargo-dist for the CLI; signed and notarised macOS app, Windows MSI, Linux AppImage and deb |
| 6.5 Store builds and releases | Play and App Store builds; tag-driven release workflow with SHA256SUMS |
| 6.6 Close-out | exit audit; the open hardware checks |

## Consequences
- The B21, B21-L2B, D11 and H1S can print. None of the four tasks has run on hardware: only the
  B21 Pro is available. protocol.md §5.2 says so.
- A D11 job with several pages relies on the last PageIndex report. If a printer does not send
  it, the job fails after 10 seconds with a stall error, after the labels are out.
- No command schema changed, so `API_VERSION` stays 3.
