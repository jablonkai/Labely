//! Rich-text span edits for the text property editor: style a selection, clear styles, and
//! keep spans on the same characters while the text is typed into.

use labely_core::document::{MAX_SPANS, TextSpan};

/// A style a selection can be toggled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Underline,
}

fn get(sp: &TextSpan, style: Style) -> Option<bool> {
    match style {
        Style::Bold => sp.bold,
        Style::Italic => sp.italic,
        Style::Underline => sp.underline,
    }
}

/// Whether every character in `start..end` has `style`, starting from the element's own
/// setting `base`.
pub fn has_style(spans: &[TextSpan], base: bool, style: Style, start: usize, end: usize) -> bool {
    (start..end).all(|i| {
        spans
            .iter()
            .filter(|sp| sp.start <= i && i < sp.end)
            .fold(base, |v, sp| get(sp, style).unwrap_or(v))
    })
}

/// Toggle `style` over `start..end`: on unless the whole range already has it. Returns
/// `None` for an empty range or when the span limit is reached.
pub fn toggle(
    spans: &[TextSpan],
    base: bool,
    style: Style,
    start: usize,
    end: usize,
) -> Option<Vec<TextSpan>> {
    if start >= end || spans.len() >= MAX_SPANS {
        return None;
    }
    let on = !has_style(spans, base, style, start, end);
    let mut sp = TextSpan {
        start,
        end,
        ..TextSpan::default()
    };
    match style {
        Style::Bold => sp.bold = Some(on),
        Style::Italic => sp.italic = Some(on),
        Style::Underline => sp.underline = Some(on),
    }
    let mut out = spans.to_vec();
    out.push(sp);
    Some(out)
}

/// Spans with `font_size_pt` set to `pt` over `start..end`.
pub fn set_size(spans: &[TextSpan], start: usize, end: usize, pt: f64) -> Option<Vec<TextSpan>> {
    if start >= end || spans.len() >= MAX_SPANS {
        return None;
    }
    let mut out = spans.to_vec();
    out.push(TextSpan {
        start,
        end,
        font_size_pt: Some(pt),
        ..TextSpan::default()
    });
    Some(out)
}

/// Spans without those overlapping `start..end`; every span when the range is empty.
pub fn clear(spans: &[TextSpan], start: usize, end: usize) -> Vec<TextSpan> {
    if start >= end {
        return Vec::new();
    }
    spans
        .iter()
        .filter(|sp| sp.end <= start || sp.start >= end)
        .cloned()
        .collect()
}

/// Move span edges after the text changed from `old` to `new`, treating the change as one
/// replaced stretch (common prefix and suffix kept). Spans that end up empty are dropped.
pub fn shift(spans: &[TextSpan], old: &str, new: &str) -> Vec<TextSpan> {
    let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_suffix)
        .take_while(|(x, y)| x == y)
        .count();
    let old_end = a.len() - suffix;
    let new_end = b.len() - suffix;
    let map = |pos: usize, is_end: bool| {
        if pos <= prefix {
            pos
        } else if pos >= old_end {
            pos - old_end + new_end
        } else if is_end {
            new_end
        } else {
            prefix
        }
    };
    spans
        .iter()
        .map(|sp| TextSpan {
            start: map(sp.start, false),
            end: map(sp.end, true),
            ..sp.clone()
        })
        .filter(|sp| sp.start < sp.end)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize) -> TextSpan {
        TextSpan {
            start,
            end,
            bold: Some(true),
            ..TextSpan::default()
        }
    }

    #[test]
    fn toggle_turns_on_then_off() {
        let on = toggle(&[], false, Style::Bold, 0, 3).unwrap();
        assert!(has_style(&on, false, Style::Bold, 0, 3));
        let off = toggle(&on, false, Style::Bold, 1, 2).unwrap();
        assert!(!has_style(&off, false, Style::Bold, 1, 2));
        assert!(has_style(&off, false, Style::Bold, 0, 1));
        assert!(toggle(&[], false, Style::Bold, 2, 2).is_none());
        assert_eq!(clear(&off, 0, 1).len(), 1);
        assert!(clear(&off, 0, 0).is_empty());
    }

    #[test]
    fn spans_follow_typing() {
        // "Hello world", "world" bold.
        let s = [span(6, 11)];
        let typed = shift(&s, "Hello world", "Hello, world");
        assert_eq!((typed[0].start, typed[0].end), (7, 12));
        let appended = shift(&s, "Hello world", "Hello world!");
        assert_eq!((appended[0].start, appended[0].end), (6, 11));
        let inside = shift(&s, "Hello world", "Hello wonderful world");
        assert_eq!((inside[0].start, inside[0].end), (6, 21));
        assert!(shift(&s, "Hello world", "Hello ").is_empty());
    }
}
