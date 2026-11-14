//! Date and time bindings: `{{now}}`, `{{now:%d.%m.%Y}}`, `{{now+30d:%Y-%m-%d}}` (ADR-0015).

use chrono::{Duration, Months, NaiveDate, NaiveDateTime};

use crate::document::Document;

/// Format used by a bare `{{now}}`.
pub const DEFAULT_FORMAT: &str = "%Y-%m-%d";

/// Variable that pins the clock, as `YYYY-MM-DD[ HH:MM[:SS]]`.
pub const CLOCK_VARIABLE: &str = "now";

/// The clock for `doc`: the `now` variable when it parses, else the local time.
pub fn clock(doc: &Document) -> NaiveDateTime {
    doc.variables
        .get(CLOCK_VARIABLE)
        .and_then(|v| parse_clock(v))
        .unwrap_or_else(|| chrono::Local::now().naive_local())
}

/// Parse `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` or `YYYY-MM-DD HH:MM:SS` (a `T` also works).
pub fn parse_clock(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim().replacen('T', " ", 1);
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M"))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
}

/// Whether `name` is a date binding (`now`, `now+1d`, `now:%H:%M`, …), valid or not.
pub fn is_date_binding(name: &str) -> bool {
    name.strip_prefix("now")
        .is_some_and(|r| r.is_empty() || r.starts_with([':', '+', '-']))
}

/// Error text for an invalid date binding.
pub fn check(name: &str) -> Result<(), String> {
    let fixed = NaiveDate::from_ymd_opt(2000, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .unwrap_or_default();
    eval(name, fixed).map(|_| ())
}

/// Evaluate a date binding at `now`. `Err` explains what is wrong.
pub fn eval(name: &str, now: NaiveDateTime) -> Result<String, String> {
    let rest = name
        .strip_prefix("now")
        .ok_or_else(|| "not a date binding".to_owned())?;
    let (offset, fmt) = match rest.split_once(':') {
        Some((o, f)) => (o.trim(), f),
        None => (rest.trim(), DEFAULT_FORMAT),
    };
    let t = if offset.is_empty() {
        now
    } else {
        apply_offset(now, offset)?
    };
    let items: Vec<_> = chrono::format::StrftimeItems::new(fmt)
        .parse()
        .map_err(|_| format!("`{fmt}` is not a valid date format"))?;
    Ok(t.format_with_items(items.iter()).to_string())
}

/// `+3d`, `-2h`, `+1M`; units s, m (minutes), h, d, w, M (months), y.
fn apply_offset(t: NaiveDateTime, o: &str) -> Result<NaiveDateTime, String> {
    let bad = || format!("offset `{o}` must look like +3d, -2h or +1M (units s m h d w M y)");
    let (sign, body) = match o.as_bytes().first() {
        Some(b'+') => (1i64, &o[1..]),
        Some(b'-') => (-1i64, &o[1..]),
        _ => return Err(bad()),
    };
    let unit = body.chars().last().ok_or_else(bad)?;
    let n: i64 = body[..body.len() - unit.len_utf8()]
        .trim()
        .parse()
        .map_err(|_| bad())?;
    let n = n.checked_mul(sign).ok_or_else(bad)?;
    let months = |m: i64| -> Option<NaiveDateTime> {
        let m32 = u32::try_from(m.unsigned_abs()).ok()?;
        if m >= 0 {
            t.checked_add_months(Months::new(m32))
        } else {
            t.checked_sub_months(Months::new(m32))
        }
    };
    let r = match unit {
        's' => Duration::try_seconds(n).and_then(|d| t.checked_add_signed(d)),
        'm' => Duration::try_minutes(n).and_then(|d| t.checked_add_signed(d)),
        'h' => Duration::try_hours(n).and_then(|d| t.checked_add_signed(d)),
        'd' => Duration::try_days(n).and_then(|d| t.checked_add_signed(d)),
        'w' => Duration::try_weeks(n).and_then(|d| t.checked_add_signed(d)),
        'M' => months(n),
        'y' => n.checked_mul(12).and_then(months),
        _ => return Err(bad()),
    };
    r.ok_or_else(|| format!("offset `{o}` is out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> NaiveDateTime {
        parse_clock("2026-01-31 08:05:09").unwrap()
    }

    #[test]
    fn formats_and_offsets() {
        assert_eq!(eval("now", at()).unwrap(), "2026-01-31");
        assert_eq!(
            eval("now:%d.%m.%Y %H:%M", at()).unwrap(),
            "31.01.2026 08:05"
        );
        assert_eq!(eval("now+1d", at()).unwrap(), "2026-02-01");
        assert_eq!(eval("now+1M", at()).unwrap(), "2026-02-28");
        assert_eq!(eval("now-2y:%Y", at()).unwrap(), "2024");
        assert_eq!(eval("now+90m:%H:%M", at()).unwrap(), "09:35");
        assert_eq!(eval("now+2w", at()).unwrap(), "2026-02-14");
        assert!(eval("now+1x", at()).is_err());
        assert!(eval("now+d", at()).is_err());
        assert!(eval("now:%Q", at()).is_err());
        assert!(check("now:%Y").is_ok());
    }

    #[test]
    fn names_and_clock() {
        assert!(is_date_binding("now"));
        assert!(is_date_binding("now+1d:%Y"));
        assert!(!is_date_binding("nowhere"));
        assert_eq!(
            parse_clock("2026-03-04").unwrap().to_string(),
            "2026-03-04 00:00:00"
        );
        assert!(parse_clock("2026-03-04T10:11").is_some());
        assert!(parse_clock("yesterday").is_none());
    }
}
