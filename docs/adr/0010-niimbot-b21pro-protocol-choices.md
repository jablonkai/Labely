# ADR-0010: Initial NIIMBOT protocol choices for B21 Pro (provisional)

- Status: proposed (to confirm on hardware in Phase 1)
- Date: 2026-11-14

## Context
protocol.md lists conflicts between the vendor SDK and niimbluelib.

## Decision
- **PrintStart.** Use the 9-byte PrintStart (vendor V6 matches niimbluelib D110M_V4).
- **SetPageSize.** Use the 13-byte SetPageSize, which niimbluelib reports working on the B21 Pro.
  The vendor's 45-byte form is the fallback.
- **Image rows.** Emit only 0x84 (blank runs) and 0x85 (bitmap rows), never 0x83 (indexed).
  Compute pixel counts per protocol.md §7.3.
- **USB serial.** Open at 115200 8N1. CDC-ACM ignores the baud, so try 230400 if needed.
- **Model selection.** Identify the model by its PrinterInfo code, then pick the variant from the
  protocol version.

## Consequences
- A Phase 1 hardware trace either confirms these choices or updates this ADR.
