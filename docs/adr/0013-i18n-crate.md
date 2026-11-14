# ADR-0013: `labely-i18n` crate

- Status: proposed
- Date: 2026-11-14

## Context
ADR-0009 picks Fluent for the studio and the CLI. Both are L4 crates and need the same bundles
and error titles. Core must stay free of UI text.

## Decision
- **Crate.** A new L0 crate, `labely-i18n`, with no workspace dependencies. It embeds
  `crates/i18n/locales/{en,hu}/{common,studio}.ftl`.
- **API.** `I18n::new(Lang)`, `Lang::detect()` (`LABELY_LANG`, `LC_ALL`, `LC_MESSAGES`,
  `LANG`), `t`, `t_args`, and `error_title(code)`. Lookups fall back to English, then to the
  key, and never panic. Bidi isolation marks are off.
- **Departure from ADR-0009.** Frontends show `error_title(code)` next to the English
  `message`. They do this instead of core adding `message_l10n` to the envelope, because the
  envelope is the agent contract and stays English.
- **Checks.** A unit test checks EN/HU key parity and that every file parses.
  `cargo xtask i18n-check` runs the same parity check in CI. The mobile string sets join it in
  Phases 4 and 5.
- **Departure from architecture §2.** The locales live inside the crate, not in a top-level
  `locales/` directory, so the crate stays self-contained.

## Consequences
- New UI text needs both an EN and a HU entry, or CI fails.
