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

    /// Canonical family name if installed (case-insensitive match).
    pub fn find_family(&self, name: &str) -> Option<String> {
        self.fs.db().faces().find_map(|f| {
            f.families
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(n, _)| n.clone())
        })
    }

    /// Shape `text` into a box `box_w` dots wide.
    pub fn layout(&mut self, text: &str, st: &TextStyle, box_w: f32) -> Laid {
        let size = st.size_px.max(0.5);
        let line_h = (size * LINE_HEIGHT * st.line_spacing.max(0.05)).max(0.5);
        let mut buf = Buffer::new(&mut self.fs, Metrics::new(size, line_h));
        buf.set_wrap(if st.wrap {
            Wrap::WordOrGlyph
        } else {
            Wrap::None
        });
        buf.set_size(Some(box_w.max(1.0)), None);
        let mut attrs = Attrs::new()
            .family(Family::Name(&st.family))
            .weight(if st.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if st.italic {
                Style::Italic
            } else {
                Style::Normal
            });
        if st.letter_spacing_px != 0.0 {
            // cosmic-text tracking is in em.
            attrs = attrs.letter_spacing(st.letter_spacing_px / size);
        }
        if st.underline {
            attrs = attrs.underline(UnderlineStyle::Single);
        }
        let align = match st.align {
            HAlign::Left => Align::Left,
            HAlign::Center => Align::Center,
            HAlign::Right => Align::Right,
        };
        buf.set_text(text, &attrs, Shaping::Advanced, Some(align));
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
        assert_eq!(substitute("plain", look), "plain");
    }

    #[test]
    fn bundled_family_is_found() {
        let e = TextEngine::new(false);
        assert_eq!(e.find_family("noto sans").as_deref(), Some(DEFAULT_FONT));
        assert!(e.find_family("Comic Sans MS").is_none());
    }
}
