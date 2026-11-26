---
name: i18n-key
description: Add, rename, reword or remove user-visible text (translation keys) in Labely across every surface and every shipped locale — studio and CLI Fluent files (crates/i18n/locales/<code>/*.ftl), Android string resources (mobile/android/app/src/main/res/values{,-<code>}/strings.xml) and iOS string catalogs (mobile/ios/Labely/Labely/*.xcstrings, every `L("key")` in Localizable.xcstrings) — with translations that follow each locale's style guide in docs/i18n/, then `cargo xtask i18n-check`. Also adds a whole new language. Use this whenever a change adds a label, button, menu item, hint, error title, dialog text or plural string, when a hard-coded string should be localised, when a key is renamed or deleted, when the app should support another language, or when `cargo xtask i18n-check`, `en_hu_parity` or `every_menu_command_has_a_label_in_both_languages` fails. Hungarian triggers too: "új fordítási kulcs", "adj hozzá egy fordítást", "fordítsd le magyarra", "lokalizáld ezt a szöveget", "nevezd át a kulcsot", "hiányzik a fordítás", "elhasalt az i18n-check", "adjunk hozzá egy új nyelvet", "legyen német fordítás is". Not for the stable English command error messages agents read (those stay English in core).
---

# Add or change a translation key

Labely ships English (the source) plus the locales listed in `docs/i18n/README.md` — today
Hungarian — on three surfaces, each with its own native format (ADR-0009, ADR-0013,
ADR-0032). Don't hard-code "en and hu": list the locales first
(`ls crates/i18n/locales`, the `values-*` folders with a `strings.xml`, the `localizations`
keys of `Localizable.xcstrings`) and write every key in all of them. CI
(`cargo xtask i18n-check`, part of `cargo xtask ci`) fails when locales drift apart, but it
only compares key *sets* — it cannot tell a wrong key in code or a clumsy translation. Those
are on you, which is why the steps below include grepping usages and following each locale's
style guide.

What stays out of translation: `labely-core` holds no UI text. Command `message`s, hints and
`CommandMeta` labels/descriptions are the English agent contract. Frontends localise only
the error title by code (`error-<code>` in `common.ftl`, `I18n::error_title`).

## 1. Decide which surfaces need the text

| Surface | Files | Used in code as |
|---|---|---|
| Studio (egui) | `crates/i18n/locales/{en,hu}/studio.ftl` | `app.i18n.t("key")`, `t_args("key", &[("count", Arg::Num(n as f64))])` |
| Studio + CLI shared | `crates/i18n/locales/{en,hu}/common.ftl` | CLI: `self.i18n.t("cli-…")`, `error_title(code)` |
| Studio menu items | `cmd-<command id, . and _ as ->` in `studio.ftl` | `menu::command_label(&app.i18n, "data.find_record")` |
| Android | `mobile/android/app/src/main/res/values/strings.xml` and `values-hu/strings.xml` | `stringResource(R.string.key)`, `pluralStringResource(…)` |
| iOS | `mobile/ios/Labely/Labely/Localizable.xcstrings` (also `InfoPlist.xcstrings`, `AppShortcuts.xcstrings`) | `L("key")`, `L("key", arg)` |

A feature on all platforms usually needs a studio key *and* the same Android/iOS key.
Only `studio.ftl` and `common.ftl` are embedded (`include_str!` in `crates/i18n/src/lib.rs`);
a new `.ftl` file would need adding there too — prefer an existing file.

## 2. Name the key like its neighbours

- **Fluent**: kebab-case with an area prefix: `menu-`, `cmd-`, `action-`, `props-`/`prop-`,
  `field-`, `data-`, `templates-`, `papers-`, `status-`, `error-`, `cli-`. Put it under the
  matching `## Section` comment in `studio.ftl`, in the same position in every locale.
- **Android and iOS share key names**: snake_case with a screen prefix (`home_new`,
  `print_grayscale_sub`, `ink_color`). iOS keys *are* the Android names (ADR-0032), so add
  the same key to both. Suffix `_sub` for subtitles, `_hint` for helper text, `_title` for
  dialog titles.
- Before inventing a key, search for an existing one with the same text
  (`grep -n "Cancel" …/values/strings.xml`); `cancel`, `save`, `delete`, `apply` already exist.

## 3. Write every locale

**English** (source): British spelling (`colour`, `Grayscale` is the existing exception),
sentence case, typographic `…` and `’` (not `...` or `'`; a raw `'` must be escaped in
Android XML).

**Each other locale**: read `docs/i18n/<code>.md` (register, label grammar, plurals,
glossary — e.g. [docs/i18n/hu.md](../../../docs/i18n/hu.md)) and the surrounding entries of
that locale, and reuse their words. A new term goes into the glossary in the same change, so
the next key uses it too. If a locale has no style guide yet, derive the terms from its
existing strings and tell the user the guide is missing.

- Plurals follow the language's CLDR categories, not English: Hungarian needs only
  `one`/`other` (often identical), Polish needs `one`/`few`/`many`/`other`. Fluent selectors,
  Android `<plurals>` and xcstrings `variations.plural` must cover them.
- Use positional placeholders (`%1$s`, `%2$d`) whenever word order may differ between
  languages.
- If you can't translate a language well, still add the key (CI requires it), say which
  values need a native speaker, and suggest running the `translation-reviewer` agent.

## 4. Edit the files

**Fluent** (`studio.ftl` example, from `data.find_record`):

```ftl
data-find = Find record…
data-find-print = Print record { $record }
data-columns = { $count ->
    [one] 1 column
   *[other] { $count } columns
}
```

**Android** — same line in both files; placeholders `%1$s`, `%1$d`:

```xml
<string name="ink_color">Ink colour</string>
<plurals name="templates_count">
    <item quantity="one">%1$d template</item>
    <item quantity="other">%1$d templates</item>
</plurals>
```

**iOS** — one entry per key in `Localizable.xcstrings` holding every locale; placeholders
`%1$@` (string) and `%1$lld` (integer); plurals go under `variations.plural.{one,other}`.
Insert at the alphabetical position, 2-space JSON indent, UTF-8 literals (no `\u` escapes),
`"state": "translated"`, as in commit f91c153:

```json
"ink_color": {
  "localizations": {
    "en": { "stringUnit": { "state": "translated", "value": "Ink colour" } },
    "hu": { "stringUnit": { "state": "translated", "value": "Tintaszín" } }
  }
},
```

(Expand to the file's multi-line layout; copy a neighbouring entry.) Validate with
`python3 -m json.tool mobile/ios/Labely/Labely/Localizable.xcstrings >/dev/null`.

## 5. Renaming or removing a key

Missing keys fail differently per surface: Android fails to compile, but Rust `t()` silently
falls back to the key text and dynamic iOS keys are not checked. So grep every usage,
including keys built at runtime:

```sh
rg -n '"old-key"|old-key' crates/studio/src crates/cli/src
rg -n 'R\.string\.old_key|R\.plurals\.old_key' mobile/android/app/src/main/kotlin
rg -n 'L\("old_key"' mobile/ios/Labely/Labely
rg -n 'format!\("(field|prop|fit|halign)-|L\("(kind|align|media|border)_\\\(' crates mobile/ios
```

The last line finds the prefix-built keys (`prop-{}`, `fit-{}`, `kind_\(type)`, `align_\(edge)`…):
renaming one key of such a family means checking every member exists in every locale.
Rename in every locale and all code in one change.

## 6. Check

```sh
cargo xtask i18n-check            # key parity with English: ftl, Android, iOS catalogs, L("key") literals
cargo test -p labely-i18n         # ftl parses, every key has a value
cargo test -p labely-studio menu  # every menu command has a cmd-* label in every language
```

`i18n-check` takes the locales from the folders of `crates/i18n/locales/` and compares each
with English on every surface, so a locale folder without its Android, iOS or `lib.rs`
counterpart fails too.

`i18n-check` reports lines such as `hu/studio.ftl: missing \`data-find\`` or
`…/HomeScreen.swift: key \`home_new\` is not in …/Localizable.xcstrings`. Fix and rerun.
It only scans Swift files directly in `mobile/ios/Labely/Labely/` and only literal `L("…")`
keys, so check dynamic keys by hand.

## 7. Adding a whole language

Follow the checklist in `docs/i18n/README.md`: style guide first (`docs/i18n/<code>.md`,
modelled on `hu.md`), then the Fluent files and the `Lang` enum in `crates/i18n/src/lib.rs`,
`values-<code>/`, the iOS `knownRegions` and catalog localisations, and the locale list in the
xtask i18n check — all in one change. Finish with the `translation-reviewer` agent and ask
for a native speaker's sign-off.
