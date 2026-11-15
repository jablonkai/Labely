//! Text layout and rasterisation with cosmic-text and the bundled Noto Sans faces.

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    UnderlineStyle, Weight, Wrap,
};
use labely_core::document::{DEFAULT_FONT, HAlign};

/// Bundled OFL-1.1 faces (see `fonts/OFL.txt`).
const FONTS: [&[u8]; 4] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSans-Bold.ttf"),
    include_bytes!("../fonts/NotoSans-Italic.ttf"),
    include_bytes!("../fonts/NotoSans-BoldItalic.ttf"),
];

/// Line height as a multiple of the font size at `line_spacing = 1`.
const LINE_HEIGHT: f32 = 1.2;

/// Text style resolved to dots.
#[derive(Debug, Clone)]
pub(crate) struct TextStyle {
    pub family: String,
    pub size_px: f32,
    pub line_spacing: f32,
    pub letter_spacing_px: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: HAlign,
    pub wrap: bool,
}

/// A stretch of text with its own style; `scale` multiplies the base font size.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub scale: f32,
}

/// A shaped buffer and its extent.
pub(crate) struct Laid {
    buf: Buffer,
    pub width: f32,
    pub height: f32,
}

pub(crate) struct TextEngine {
    fs: FontSystem,
    cache: SwashCache,
}

impl std::fmt::Debug for TextEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextEngine")
            .field("faces", &self.fs.db().len())
            .finish()
    }
}

impl TextEngine {
    pub fn new(system_fonts: bool) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        for f in FONTS {
            db.load_font_data(f.to_vec());
        }
        if system_fonts {
            db.load_system_fonts();
        }
        db.set_sans_serif_family(DEFAULT_FONT);
        Self {
            fs: FontSystem::new_with_locale_and_db("en-US".to_owned(), db),
            cache: SwashCache::new(),
        }
    }

    /// Load one font file (TTF, OTF, TTC). Returns the families it added, sorted.
    pub fn add_font(&mut self, bytes: Vec<u8>) -> Result<Vec<String>, String> {
        let magic = bytes.get(..4).unwrap_or_default();
        let known = matches!(magic, [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf");
        if !known {
            return Err("not a TrueType or OpenType font (TTF, OTF, TTC)".to_owned());
        }
        let db = self.fs.db_mut();
        let before: std::collections::HashSet<_> = db.faces().map(|f| f.id).collect();
        db.load_font_data(bytes);
        let mut added: Vec<String> = db
            .faces()
            .filter(|f| !before.contains(&f.id))
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        if added.is_empty() {
            return Err("the font file has no readable faces".to_owned());
        }
        added.sort_by_key(|n| n.to_lowercase());
        added.dedup();
        Ok(added)
    }

    /// Canonical family name if installed (case-insensitive match).
    pub fn find_family(&self, name: &str) -> Option<String> {
        self.fs.db().faces().find_map(|f| {
            f.families
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(n, _)| n.clone())
        })
    }

    /// Every installed family name, sorted case-insensitively and without duplicates.
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .fs
            .db()
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        out.sort_by_key(|n| n.to_lowercase());
        out.dedup();
        out
    }

    /// Shape `text` into a box `box_w` dots wide.
    pub fn layout(&mut self, text: &str, st: &TextStyle, box_w: f32) -> Laid {
        let run = Run {
            text: text.to_owned(),
            bold: st.bold,
            italic: st.italic,
            underline: st.underline,
            scale: 1.0,
        };
        self.layout_runs(std::slice::from_ref(&run), st, box_w)
    }

    /// Shape styled `runs` into a box `box_w` dots wide; `st` gives the base size and the
    /// paragraph settings.
    pub fn layout_runs(&mut self, runs: &[Run], st: &TextStyle, box_w: f32) -> Laid {
        let size = st.size_px.max(0.5);
        let spacing = LINE_HEIGHT * st.line_spacing.max(0.05);
        let line_h = (size * spacing).max(0.5);
        let mut buf = Buffer::new(&mut self.fs, Metrics::new(size, line_h));
        buf.set_wrap(if st.wrap {
            Wrap::WordOrGlyph
        } else {
            Wrap::None
        });
        buf.set_size(Some(box_w.max(1.0)), None);
        let attrs_for = |bold: bool, italic: bool, underline: bool, scale: f32| {
            let size = (size * scale).max(0.5);
            let mut a = Attrs::new()
                .family(Family::Name(&st.family))
                .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
                .style(if italic { Style::Italic } else { Style::Normal });
            if scale != 1.0 {
                a = a.metrics(Metrics::new(size, (size * spacing).max(0.5)));
            }
            if st.letter_spacing_px != 0.0 {
                // cosmic-text tracking is in em.
                a = a.letter_spacing(st.letter_spacing_px / size);
            }
            if underline {
                a = a.underline(UnderlineStyle::Single);
            }
            a
        };
        let base = attrs_for(st.bold, st.italic, st.underline, 1.0);
        let align = match st.align {
            HAlign::Left => Align::Left,
            HAlign::Center => Align::Center,
            HAlign::Right => Align::Right,
            HAlign::Justify => Align::Justified,
        };
        let spans: Vec<(&str, Attrs<'_>)> = runs
            .iter()
            .map(|r| {
                (
                    r.text.as_str(),
                    attrs_for(r.bold, r.italic, r.underline, r.scale),
                )
            })
            .collect();
        buf.set_rich_text(spans, &base, Shaping::Advanced, Some(align));
        buf.shape_until_scroll(&mut self.fs, false);
        let (mut width, mut height) = (0.0f32, 0.0f32);
        for run in buf.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        Laid { buf, width, height }
    }

    /// Rasterise `laid` into a coverage buffer (`w`×`h`, 0‥255) at integer offset `(dx, dy)`.
    pub fn draw(&mut self, laid: &mut Laid, dx: i32, dy: i32, cov: &mut [u8], w: u32, h: u32) {
        let (w, h) = (w as i32, h as i32);
        laid.buf.draw(
            &mut self.fs,
            &mut self.cache,
            Color::rgb(0, 0, 0),
            |x, y, gw, gh, c| {
                let a = c.a();
                if a == 0 {
                    return;
                }
                for py in (y + dy).max(0)..(y + dy + gh as i32).min(h) {
                    for px in (x + dx).max(0)..(x + dx + gw as i32).min(w) {
                        let i = (py * w + px) as usize;
                        if let Some(slot) = cov.get_mut(i) {
                            *slot = (*slot).max(a);
                        }
                    }
                }
            },
        );
    }
}

/// Stack characters one per line for vertical text; source lines are separated by a blank line.
pub fn vertical(s: &str) -> String {
    s.lines()
        .map(|l| {
            l.chars()
                .filter(|c| !c.is_whitespace() || *c == ' ')
                .map(String::from)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Replace `{{name}}` with a value from `lookup`; unknown names are left untouched.
pub(crate) fn substitute<'a>(text: &str, lookup: impl Fn(&str) -> Option<&'a str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim();
                match lookup(name) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitution() {
        let look = |n: &str| match n {
            "a" => Some("1"),
            "name" => Some("Ada"),
            _ => None,
        };
        assert_eq!(substitute("x{{a}}y{{ name }}z", look), "x1yAdaz");
        assert_eq!(substitute("{{missing}} {{a", look), "{{missing}} {{a");
        assert_eq!(vertical("ab\nc"), "a\nb\n\nc");
        assert_eq!(substitute("plain", look), "plain");
    }

    /// Right edge of the first line's last glyph.
    fn first_line_end(align: HAlign) -> f32 {
        let mut e = TextEngine::new(false);
        let st = TextStyle {
            family: DEFAULT_FONT.into(),
            size_px: 20.0,
            line_spacing: 1.0,
            letter_spacing_px: 0.0,
            bold: false,
            italic: false,
            underline: false,
            align,
            wrap: true,
        };
        let laid = e.layout("aa bbb c dddd ee fff gg hhhh ii jj kkk", &st, 200.0);
        laid.buf
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.last().map(|g| g.x + g.w))
            .unwrap_or(0.0)
    }

    #[test]
    fn justify_fills_wrapped_lines() {
        let left = first_line_end(HAlign::Left);
        let justified = first_line_end(HAlign::Justify);
        assert!(left < 195.0, "left-aligned line should be ragged: {left}");
        assert!(
            justified > 195.0 && justified <= 200.5,
            "justified: {justified}"
        );
    }

    #[test]
    fn add_font_checks_the_file() {
        let mut e = TextEngine::new(false);
        assert!(e.add_font(b"not a font".to_vec()).is_err());
        assert!(e.add_font(Vec::new()).is_err());
        // Right magic, no tables.
        assert!(e.add_font(vec![0, 1, 0, 0, 0, 0]).is_err());
        let faces = e.fs.db().len();
        assert_eq!(
            e.add_font(FONTS[1].to_vec()),
            Ok(vec![DEFAULT_FONT.to_owned()])
        );
        assert_eq!(e.fs.db().len(), faces + 1);
    }

    #[test]
    fn bundled_family_is_found() {
        let e = TextEngine::new(false);
        assert_eq!(e.find_family("noto sans").as_deref(), Some(DEFAULT_FONT));
        assert!(e.find_family("Comic Sans MS").is_none());
    }
}
