//! Labely localisation (ADR-0009, ADR-0012): Fluent bundles for the studio and the CLI.
//!
//! Messages live in `locales/<lang>/*.ftl` and are embedded at build time. Lookups fall back
//! to English, then to the key itself, so a missing string never panics. Command error
//! messages stay English for agents; frontends show [`I18n::error_title`] next to them.

use std::collections::BTreeSet;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    En,
    Hu,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Hu];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Hu => "hu",
        }
    }

    /// Native name, for a language picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Hu => "Magyar",
        }
    }

    /// Parse `hu`, `hu_HU.UTF-8`, `en-GB`, …; unknown languages give `None`.
    pub fn parse(s: &str) -> Option<Lang> {
        let lang = s
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match lang.as_str() {
            "en" => Some(Lang::En),
            "hu" => Some(Lang::Hu),
            _ => None,
        }
    }

    /// From `LABELY_LANG`, then `LC_ALL`, `LC_MESSAGES`, `LANG`; default English.
    pub fn detect() -> Lang {
        ["LABELY_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find_map(|v| Lang::parse(&v))
            .unwrap_or_default()
    }

    fn sources(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[
                include_str!("../locales/en/studio.ftl"),
                include_str!("../locales/en/common.ftl"),
            ],
            Lang::Hu => &[
                include_str!("../locales/hu/studio.ftl"),
                include_str!("../locales/hu/common.ftl"),
            ],
        }
    }
}

type Bundle = FluentBundle<FluentResource>;

fn bundle(lang: Lang) -> Bundle {
    let id: LanguageIdentifier = lang.code().parse().unwrap_or_default();
    let mut b = Bundle::new_concurrent(vec![id]);
    b.set_use_isolating(false);
    for src in lang.sources() {
        // Syntax errors keep the valid entries; the parity test catches broken files.
        let res = FluentResource::try_new((*src).to_owned()).unwrap_or_else(|(r, _)| r);
        // Duplicate ids are a bug caught by tests; keep the first definition.
        let _ = b.add_resource(res);
    }
    b
}

/// Message lookup for one language with English fallback.
pub struct I18n {
    lang: Lang,
    main: Bundle,
    fallback: Option<Bundle>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n").field("lang", &self.lang).finish()
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Lang::En)
    }
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            main: bundle(lang),
            fallback: (lang != Lang::En).then(|| bundle(Lang::En)),
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// The message `key`, or the key itself when missing.
    pub fn t(&self, key: &str) -> String {
        self.format(key, None)
    }

    /// The message `key` with `{ $name }` arguments.
    pub fn t_args(&self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            let v: FluentValue<'_> = match v {
                Arg::Str(s) => (*s).into(),
                Arg::Num(n) => (*n).into(),
            };
            fa.set(*k, v);
        }
        self.format(key, Some(&fa))
    }

    /// Localised title for a command error code (`bad_params`, `not_found`, …).
    pub fn error_title(&self, code: &str) -> String {
        let key = format!("error-{}", code.replace('_', "-"));
        if self.has(&key) {
            self.t(&key)
        } else {
            self.t("error-internal")
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.main.has_message(key) || self.fallback.as_ref().is_some_and(|b| b.has_message(key))
    }

    fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        for b in std::iter::once(&self.main).chain(self.fallback.as_ref()) {
            if let Some(pattern) = b.get_message(key).and_then(|m| m.value()) {
                let mut errors = Vec::new();
                return b.format_pattern(pattern, args, &mut errors).into_owned();
            }
        }
        key.to_owned()
    }
}

/// A Fluent argument value.
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    Str(&'a str),
    Num(f64),
}

/// Message ids defined for `lang` (for the EN/HU parity check).
pub fn keys(lang: Lang) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for src in lang.sources() {
        for line in src.lines() {
            let Some((id, _)) = line.split_once('=') else {
                continue;
            };
            let id = id.trim_end();
            let starts_ok = id.chars().next().is_some_and(|c| c.is_ascii_lowercase());
            if starts_ok
                && !line.starts_with(char::is_whitespace)
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                out.insert(id.to_owned());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_hu_parity() {
        let en = keys(Lang::En);
        let hu = keys(Lang::Hu);
        let missing: Vec<_> = en.difference(&hu).collect();
        let extra: Vec<_> = hu.difference(&en).collect();
        assert!(missing.is_empty(), "missing in hu: {missing:?}");
        assert!(extra.is_empty(), "only in hu: {extra:?}");
        assert!(en.len() > 20);
    }

    #[test]
    fn files_parse_cleanly() {
        for lang in Lang::ALL {
            for src in lang.sources() {
                if let Err((_, errs)) = FluentResource::try_new((*src).to_owned()) {
                    panic!("{}: {errs:?}", lang.code());
                }
            }
            let b = I18n::new(lang);
            for k in keys(lang) {
                assert_ne!(b.t(&k), k, "{}: `{k}` has no value", lang.code());
            }
        }
    }

    #[test]
    fn lookup_and_fallback() {
        let hu = I18n::new(Lang::Hu);
        assert_eq!(hu.t("menu-file"), "Fájl");
        assert_eq!(hu.t("no-such-key"), "no-such-key");
        assert_eq!(
            hu.t_args("status-elements", &[("count", Arg::Num(3.0))]),
            "3 elem"
        );
        let en = I18n::new(Lang::En);
        assert_eq!(
            en.t_args("status-elements", &[("count", Arg::Num(1.0))]),
            "1 element"
        );
        assert_eq!(en.error_title("not_found"), "Not found");
        assert_eq!(Lang::parse("hu_HU.UTF-8"), Some(Lang::Hu));
        assert_eq!(Lang::parse("de_DE"), None);
    }
}
