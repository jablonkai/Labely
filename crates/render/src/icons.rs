//! Built-in icons: original 24×24 SVGs drawn for Labely (MIT OR Apache-2.0), rendered with
//! resvg. Names are stable; add new icons at the end of their category.

use tiny_skia::{Pixmap, Transform};

/// Wrap SVG body markup in a 24×24 black-stroked document.
macro_rules! svg {
    ($body:expr) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
            $body,
            "</svg>"
        )
    };
}

/// (name, category, svg)
pub(crate) const ICONS: &[(&str, &str, &str)] = &[
    // Handling marks.
    (
        "fragile",
        "handling",
        svg!(
            r#"<path d="M7 2h10l-.6 6.5a4.4 4.4 0 0 1-8.8 0z"/><path d="M12 13v7M8 21h8"/><path d="M11 2l1.5 3-2 2" stroke-width="1.5"/>"#
        ),
    ),
    (
        "keep_dry",
        "handling",
        svg!(
            r#"<path d="M2.5 12a9.5 8 0 0 1 19 0z"/><path d="M12 12v7a2 2 0 0 1-4 0"/><path d="M12 2.5v1.5"/><path d="M5 2l-1 2.5M19 2l1 2.5" stroke-width="1.5"/>"#
        ),
    ),
    (
        "this_way_up",
        "handling",
        svg!(r#"<path d="M8 17V4M4.5 7.5 8 4l3.5 3.5M16 17V4M12.5 7.5 16 4l3.5 3.5M3 21h18"/>"#),
    ),
    (
        "keep_cold",
        "handling",
        svg!(
            r#"<path d="M12 2v20M3.3 7l17.4 10M3.3 17 20.7 7"/><path d="M9.5 3.5 12 6l2.5-2.5M9.5 20.5 12 18l2.5 2.5" stroke-width="1.5"/>"#
        ),
    ),
    (
        "keep_away_from_sun",
        "handling",
        svg!(
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22M4.9 4.9l1.8 1.8M17.3 17.3l1.8 1.8M4.9 19.1l1.8-1.8M17.3 6.7l1.8-1.8"/><path d="M3 3l18 18"/>"#
        ),
    ),
    (
        "do_not_stack",
        "handling",
        svg!(
            r#"<rect x="6" y="13" width="12" height="8"/><rect x="6" y="3" width="12" height="7" stroke-dasharray="2 2"/><path d="M3 3l18 18"/>"#
        ),
    ),
    // Safety.
    (
        "warning",
        "safety",
        svg!(
            r#"<path d="M12 2.5 22 20H2z"/><path d="M12 9v5"/><circle cx="12" cy="17" r=".6" fill="black"/>"#
        ),
    ),
    (
        "flammable",
        "safety",
        svg!(
            r#"<path d="M12 22c-4 0-7-2.6-7-6.5 0-3.5 2.5-5.5 3.5-8.5.5 2 1.5 3 2.5 3.5C11 7 12 4 15 2c0 4 4 6.5 4 12 0 4.5-3 8-7 8z"/><path d="M12 22c-1.7 0-3-1.2-3-3 0-2 1.5-3 2-4.5.5 1 1 1.5 1.5 1.5.3 1.4 2.5 2 2.5 3.5 0 1.5-1.3 2.5-3 2.5z" stroke-width="1.5"/>"#
        ),
    ),
    (
        "electricity",
        "safety",
        svg!(r#"<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>"#),
    ),
    (
        "no_entry",
        "safety",
        svg!(r#"<circle cx="12" cy="12" r="10"/><path d="M5 12h14" stroke-width="3"/>"#),
    ),
    (
        "prohibited",
        "safety",
        svg!(r#"<circle cx="12" cy="12" r="10"/><path d="M5 5l14 14"/>"#),
    ),
    (
        "first_aid",
        "safety",
        svg!(
            r#"<rect x="2.5" y="2.5" width="19" height="19" rx="3"/><path d="M12 7v10M7 12h10" stroke-width="3"/>"#
        ),
    ),
    // Symbols.
    (
        "check",
        "symbols",
        svg!(r#"<path d="M4 12.5 9.5 18 20 6"/>"#),
    ),
    (
        "cross",
        "symbols",
        svg!(r#"<path d="M5 5l14 14M19 5 5 19"/>"#),
    ),
    (
        "star",
        "symbols",
        svg!(
            r#"<path d="M12 2.5l2.9 6.2 6.6.8-4.9 4.6 1.3 6.6L12 17.4l-5.9 3.3 1.3-6.6-4.9-4.6 6.6-.8z"/>"#
        ),
    ),
    (
        "heart",
        "symbols",
        svg!(
            r#"<path d="M12 20.5S3 15 3 8.8A4.8 4.8 0 0 1 12 6.3a4.8 4.8 0 0 1 9 2.5C21 15 12 20.5 12 20.5z"/>"#
        ),
    ),
    (
        "arrow_right",
        "symbols",
        svg!(r#"<path d="M3 12h17M14 6l6 6-6 6"/>"#),
    ),
    (
        "arrow_up",
        "symbols",
        svg!(r#"<path d="M12 21V4M6 10l6-6 6 6"/>"#),
    ),
    (
        "info",
        "symbols",
        svg!(
            r#"<circle cx="12" cy="12" r="10"/><path d="M12 11v6"/><circle cx="12" cy="7.5" r=".6" fill="black"/>"#
        ),
    ),
    (
        "recycle",
        "symbols",
        svg!(
            r#"<path d="M7.5 9 10 4.6a2 2 0 0 1 3.5 0l1.8 3.1"/><path d="M13.3 7.9l2 .2.6-2"/><path d="M18.5 12.4l2.2 3.9a2 2 0 0 1-1.7 3H15.5"/><path d="M17.3 20.6l-1.8-1.3 1.2-1.8"/><path d="M9 19.3H4.9a2 2 0 0 1-1.7-3l1.9-3.3"/><path d="M3.3 13.7l1.8-.7.9 1.9"/>"#
        ),
    ),
    // Office and contact.
    (
        "phone",
        "office",
        svg!(
            r#"<path d="M5 3h4l2 5-2.5 1.5a11 11 0 0 0 6 6L16 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 5a2 2 0 0 1 2-2z"/>"#
        ),
    ),
    (
        "mail",
        "office",
        svg!(r#"<rect x="2.5" y="5" width="19" height="14" rx="1.5"/><path d="M3 6l9 7 9-7"/>"#),
    ),
    (
        "home",
        "office",
        svg!(r#"<path d="M3 11 12 3l9 8"/><path d="M5 9.5V21h5v-6h4v6h5V9.5"/>"#),
    ),
    (
        "location",
        "office",
        svg!(
            r#"<path d="M12 22s7-6.6 7-12.5a7 7 0 0 0-14 0C5 15.4 12 22 12 22z"/><circle cx="12" cy="9.5" r="2.5"/>"#
        ),
    ),
    (
        "clock",
        "office",
        svg!(r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2.5"/>"#),
    ),
    (
        "calendar",
        "office",
        svg!(
            r#"<rect x="3" y="4.5" width="18" height="17" rx="1.5"/><path d="M3 9.5h18M8 2.5v4M16 2.5v4"/>"#
        ),
    ),
    (
        "user",
        "office",
        svg!(r#"<circle cx="12" cy="8" r="4.5"/><path d="M3.5 21.5a8.5 8.5 0 0 1 17 0"/>"#),
    ),
    // Food and kitchen.
    (
        "leaf",
        "food",
        svg!(r#"<path d="M4 20C4 9 10 4 21 3c-.5 11-6 17-16 16"/><path d="M4 20 13 11"/>"#),
    ),
    (
        "hot_drink",
        "food",
        svg!(
            r#"<path d="M4 9h13v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5z"/><path d="M17 11h1.5a2.5 2.5 0 0 1 0 5H16.5"/><path d="M8 2.5c-1 1.2 1 2.3 0 3.5M12.5 2.5c-1 1.2 1 2.3 0 3.5" stroke-width="1.5"/>"#
        ),
    ),
    (
        "cutlery",
        "food",
        svg!(
            r#"<path d="M6 2v7a2 2 0 0 0 4 0V2M8 9v13"/><path d="M17 22V2c-2.5 1.5-3.5 4-3.5 8v3H17"/>"#
        ),
    ),
    (
        "freezer",
        "food",
        svg!(
            r#"<rect x="4" y="2" width="16" height="20" rx="2"/><path d="M4 9h16M8 5v2M8 12v4"/>"#
        ),
    ),
];

/// Render `name` into a `w`×`h` transparent pixmap (black ink), or `None` if unknown.
pub(crate) fn render(name: &str, w: u32, h: u32, keep_aspect: bool) -> Option<Pixmap> {
    let svg = ICONS.iter().find(|(n, _, _)| *n == name)?.2;
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let (sx, sy) = (w as f32 / size.width(), h as f32 / size.height());
    let (sx, sy, dx, dy) = if keep_aspect {
        let s = sx.min(sy);
        (
            s,
            s,
            (w as f32 - size.width() * s) / 2.0,
            (h as f32 - size.height() * s) / 2.0,
        )
    } else {
        (sx, sy, 0.0, 0.0)
    };
    let mut pm = Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        Transform::from_row(sx, 0.0, 0.0, sy, dx, dy),
        &mut pm.as_mut(),
    );
    Some(pm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_parses_and_draws() {
        let mut names = std::collections::BTreeSet::new();
        for (name, _, _) in ICONS {
            assert!(names.insert(*name), "duplicate {name}");
            let pm = render(name, 48, 48, true).unwrap_or_else(|| panic!("{name}"));
            let ink = pm.data().chunks(4).filter(|p| p[3] > 128).count();
            assert!(ink > 40, "{name} draws almost nothing ({ink})");
        }
        assert!(render("nope", 10, 10, true).is_none());
    }
}
