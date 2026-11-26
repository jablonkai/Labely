---
paths:
  - "crates/protocol/**"
  - "crates/printers/**"
  - "crates/transport/**"
---

# Clean room (ADR-0001)

Labely is an independent alternative to vendor apps; its legal safety depends on how protocol
knowledge enters the code. Rules here come from `docs/adr/0001-record-decisions-and-clean-room.md`.

- **Spec first.** Product code is written from the specs only: `docs/protocol.md`,
  `docs/protocol-languages.md`, `docs/printers.md` (and `docs/feature-matrix.md`). Update the spec
  before, or in the same change as, the code. Every protocol behaviour in code must trace to a
  spec section, so cite the section in a comment when the behaviour is not obvious.
- **Never** copy vendor code, assets, templates, fonts, icons or UI strings.
- **Never** call vendor cloud APIs.
- **Never** investigate or bypass licence, VIP or consumable-authenticity checks.
- **Open source.** niimprint, niimbluelib and NiimBlue (MIT) are documentation to cross-check.
  Cite facts taken from them in the spec; do not copy their code.
- **Captures** of the vendor app with the user's own printer are for observing behaviour, stored
  as normalised fixtures (e.g. `crates/core/tests/fixtures/`).
- **Reverse engineering** (`.claude/skills/reverse-engineer-anything`) produces written specs with
  cited evidence, never code. Its output lands in the docs above, and code follows from there.
- **Layers (ADR-0003).** protocol is L0, transport L1 (no workspace deps, the only crate with
  btleplug), printers L2. Adding a vendor touches only protocol and printers. `cargo xtask layers`
  enforces this.

Before merging, run the `clean-room-reviewer` agent on the diff.
