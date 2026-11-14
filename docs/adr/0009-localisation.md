# ADR-0009: Localisation

- Status: proposed
- Date: 2026-11-14

## Decision
- **Rust surfaces.** The studio and CLI use Fluent (`fluent-bundle` 0.16), with files in
  `locales/{en,hu}/`.
- **Command errors.** Error `message` stays stable English for agents. `message_l10n` carries the
  localised text for display.
- **Mobile shells.** Native resources (`strings.xml` and String Catalogs) hold the UI strings.
- **Parity check.** `xtask i18n-check` checks that EN and HU have the same keys.
- **Rejected.** rust-i18n's macro and YAML approach. Fluent handles Hungarian plurals and cases
  better.

## Consequences
- Adding a language means adding one folder per surface.
