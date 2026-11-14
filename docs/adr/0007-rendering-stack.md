# ADR-0007: Rendering stack

- Status: proposed
- Date: 2026-11-14

## Decision
- **Rasterisation.** tiny-skia 0.12 draws to 8-bit gray at the model's dpi.
- **Text.** cosmic-text 0.19 handles shaping, fallback, wrapping and auto-shrink.
- **Codes.** rxing 0.9 encodes and decodes 1D codes, QR, PDF417 and DataMatrix. Code modules are
  snapped to whole dots.
- **Images.** image 0.25 and imageproc 0.27.
- **1-bit conversion.** Done per element, with threshold, Floyd–Steinberg, Atkinson or Bayer.
- **Output.** `Bitmap1`, already rotated to the model's print direction.
- **Determinism.** Rendering must be byte-identical for the same input. Goldens use only bundled
  OFL fonts.
- **Readability check.** The `doc.check` round-trip decodes codes with the same rxing build that
  encoded them.

## Consequences
- Pure Rust, with no C++ toolchain for mobile cross-builds.
- zxing-cpp and barcoders were rejected: zxing-cpp needs a C++ build, and barcoders is 1D-only and
  stale.
- rxing's encoders still have to be checked per symbology (Codabar, GS1-128) in Phase 1. Any gap
  gets its own encoder written from the published standard.
