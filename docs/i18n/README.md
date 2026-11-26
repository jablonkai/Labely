# Localisation

English is the source language. Every other locale must have the same keys as English on every
surface (ADR-0009, ADR-0013, ADR-0032), which `cargo xtask i18n-check` checks.

## Shipped locales

| Code | Language | Style guide |
|---|---|---|
| `en` | English (source) | British spelling, sentence case, typographic `…` and `’` |
| `hu` | Hungarian | [hu.md](hu.md) |

Every non-English locale has a style guide `docs/i18n/<code>.md`: register (formal or informal
"you"), label grammar, plural notes and a glossary of Labely terms. Translators, the `i18n-key`
skill and the `translation-reviewer` agent all work from it, so terms stay consistent.

## Where the strings live

| Surface | Files |
|---|---|
| Studio and CLI (Fluent) | `crates/i18n/locales/<code>/{studio,common}.ftl` |
| Android | `mobile/android/app/src/main/res/values-<code>/strings.xml` (English: `values/`) |
| iOS | `localizations.<code>` in `mobile/ios/Labely/Labely/{Localizable,InfoPlist,AppShortcuts}.xcstrings` |

## Adding a locale

Do it in one change, so no surface ships half a language.

1. **Style guide.** Copy [hu.md](hu.md) to `docs/i18n/<code>.md` and rewrite it for the
   language. Settle the register and the glossary before translating, then add the row above.
2. **Fluent.** Add `crates/i18n/locales/<code>/studio.ftl` and `common.ftl` with every English
   key. In `crates/i18n/src/lib.rs`, add the `Lang` variant, `Lang::ALL`, `code`,
   `native_name`, `parse` and the `include_str!` pair in `sources`.
3. **Android.** Add `values-<code>/strings.xml` with every key, plurals included. The locale
   config is generated (`generateLocaleConfig = true` in `app/build.gradle.kts`).
4. **iOS.** Add `<code>` to `knownRegions` in `mobile/ios/Labely/Labely.xcodeproj/project.pbxproj`
   and a `<code>` localisation to every entry in all three `.xcstrings` catalogs.
5. **Check.** `cargo xtask i18n-check` takes the locales from `crates/i18n/locales/` and
   reports every step above that is missing: the `include_str!` in `lib.rs`, keys in the
   Fluent files, the `values-<code>` folder (`pt-BR` → `values-pt-rBR`), `knownRegions` and
   the catalog values. Then run `cargo test -p labely-i18n` and
   `cargo test -p labely-studio menu`.
6. **Review.** Run the `translation-reviewer` agent on the new locale. A native speaker should
   sign it off before release.

Plurals follow the CLDR categories of each language (for example `one`/`other` in English,
`one`/`few`/`many`/`other` in Polish). Fluent selectors, Android `<plurals>` and xcstrings
`variations.plural` must list the categories the language needs, not just copy English.
