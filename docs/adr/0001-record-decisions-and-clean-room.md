# ADR-0001: Record decisions as ADRs; clean-room policy

- Status: proposed
- Date: 2026-11-14

## Context
Labely is an independent alternative to the NIIMBOT app. Its legal safety depends on how protocol
knowledge is obtained and used.

## Decision
- **ADRs.** Every significant decision is an ADR in `docs/adr/NNNN-slug.md`, using `0000-template.md`.
- **Clean room.** Reverse-engineering produces written specs only: `protocol.md`, `printers.md` and
  `feature-matrix.md`, with cited evidence. Product code is written from those specs.
- **What we never do:**
  - copy vendor code, assets, templates, fonts, icons or UI strings;
  - call vendor cloud APIs;
  - investigate or bypass licence, VIP or consumable-authenticity checks.
- **Open-source references.** niimprint, niimbluelib and NiimBlue (all MIT) are cross-checked as
  documentation. Their code is not copied. Facts taken from them are cited.
- **Traffic captures.** Captures of the vendor app talking to the user's own printer are used to
  observe behaviour only, and are stored as normalised fixtures.

## Consequences
- Every protocol behaviour in code must trace back to a spec section.
- Spec updates come before code changes.
