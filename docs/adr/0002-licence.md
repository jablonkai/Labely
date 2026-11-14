# ADR-0002: Licence MIT OR Apache-2.0

- Status: proposed
- Date: 2026-11-14

## Decision
- **Licence.** Dual licence MIT OR Apache-2.0, the Rust ecosystem norm. Both licence files are in
  the repo root.
- **Dependencies.** Must be compatible: MIT, Apache-2.0, BSD, Zlib, ISC, Unicode or MPL-2.0 at file
  level. No GPL or AGPL in shipped binaries. `cargo-deny` enforces this in CI.
- **Fonts.** Bundled fonts are OFL-1.1.
- **Icons and templates.** Original, under the project licence or CC0.

## Consequences
- dssim (AGPL) is excluded, so golden-image comparison uses exact 1-bit equality or `image-compare`.
- rxing is Apache-2.0, which is compatible.
