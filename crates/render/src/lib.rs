//! labely-render: rasterises a [`Document`] page to a 1-bit [`Bitmap1`] at printer dpi
//! (ADR-0007).
//!
//! Pipeline: label mm → dots; each visible element is drawn (frame + rotation about its
//! centre) into its own 8-bit RGBA layer with tiny-skia, converted to 1-bit (threshold for
//! text/shapes/codes, the element's [`Dither`] for images) and composed in z-order.
//! Shapes and codes use hard edges; text uses only the bundled Noto Sans faces by default, so
//! output is deterministic.

mod codes;
mod dither;
mod icons;
mod shapes;
mod text;

use std::sync::{Mutex, PoisonError};

use base64::Engine as _;
use labely_core::document::{
    BarcodeProps, Code2dProps, DEFAULT_FONT, Dither, Element, ElementKind, HumanReadable, ImageFit,
    ImageProps, Symbology2d, TableProps, TextDirection, TextFit, TextProps, VAlign,
};
use labely_core::raster::Bitmap1;
use labely_core::services::{
    DecodedCode, RenderError, RenderIssue, RenderOptions, RenderOutput, Renderer,
};
use labely_core::{Document, document::HAlign};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub use dither::dither;
use text::{TextEngine, TextStyle};

/// Largest accepted label side in dots.
const MAX_DOTS: i64 = 32_768;

/// Millimetres to whole dots: `round(mm / 25.4 * dpi)`.
pub fn mm_to_dots(mm: f64, dpi: u32) -> i64 {
    (mm / 25.4 * f64::from(dpi)).round() as i64
}

/// Points to (fractional) dots: `pt / 72 * dpi`.
pub fn pt_to_dots(pt: f64, dpi: u32) -> f64 {
    pt / 72.0 * f64::from(dpi)
}

/// 8-bit gray preview (row-major, 0 = black, 255 = white) before 1-bit conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The production [`Renderer`].
#[derive(Debug)]
pub struct LabelRenderer {
    text: Mutex<TextEngine>,
}

impl Default for LabelRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl LabelRenderer {
    /// Renderer using only the bundled fonts (deterministic).
    pub fn new() -> Self {
        Self {
            text: Mutex::new(TextEngine::new(false)),
        }
    }

    /// Renderer that also loads system fonts. Output then depends on the machine.
    pub fn with_system_fonts() -> Self {
        Self {
            text: Mutex::new(TextEngine::new(true)),
        }
    }

    /// Render and also return the 8-bit gray preview.
    pub fn render_with_gray(
        &self,
        doc: &Document,
        opts: &RenderOptions,
    ) -> Result<(RenderOutput, GrayImage), RenderError> {
        let mut engine = self.text.lock().unwrap_or_else(PoisonError::into_inner);
        render_page(&mut engine, doc, opts)
    }

    /// 8-bit gray preview of the page.
    pub fn render_gray(
        &self,
        doc: &Document,
        opts: &RenderOptions,
    ) -> Result<GrayImage, RenderError> {
        self.render_with_gray(doc, opts).map(|(_, g)| g)
    }
}

impl Renderer for LabelRenderer {
    fn render(&self, doc: &Document, opts: &RenderOptions) -> Result<RenderOutput, RenderError> {
        self.render_with_gray(doc, opts).map(|(o, _)| o)
    }

    fn decode_codes(&self, bitmap: &Bitmap1) -> Vec<DecodedCode> {
        codes::decode(bitmap)
    }

    fn to_png(&self, bitmap: &Bitmap1) -> Result<Vec<u8>, RenderError> {
        bitmap_to_png(bitmap)
    }

    fn icons(&self) -> Vec<labely_core::services::IconInfo> {
        icons::ICONS
            .iter()
            .map(|(name, category, _)| labely_core::services::IconInfo {
                name: (*name).to_owned(),
                category: (*category).to_owned(),
            })
            .collect()
    }

    fn decode_image(&self, bytes: &[u8]) -> Result<Vec<DecodedCode>, RenderError> {
        codes::decode_image(bytes).map_err(RenderError::Other)
    }
}

/// Encode a [`Bitmap1`] as a 1-bit grayscale PNG (black = 0).
pub fn bitmap_to_png(bitmap: &Bitmap1) -> Result<Vec<u8>, RenderError> {
    let err = |e: png::EncodingError| RenderError::Other(format!("png: {e}"));
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, bitmap.width(), bitmap.height());
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::One);
    let mut w = enc.write_header().map_err(err)?;
    let data: Vec<u8> = bitmap.data().iter().map(|b| !b).collect();
    w.write_image_data(&data).map_err(err)?;
    w.finish().map_err(err)?;
    Ok(out)
}

/// Encode a [`GrayImage`] as an 8-bit grayscale PNG.
pub fn gray_to_png(img: &GrayImage) -> Result<Vec<u8>, RenderError> {
    let err = |e: png::EncodingError| RenderError::Other(format!("png: {e}"));
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(err)?;
    w.write_image_data(&img.data).map_err(err)?;
    w.finish().map_err(err)?;
    Ok(out)
}

/// How a layer becomes 1-bit.
#[derive(Debug, Clone, Copy)]
enum Conv {
    Threshold,
    Dither(Dither, u8),
}

fn pixmap(w: u32, h: u32) -> Result<Pixmap, RenderError> {
    Pixmap::new(w, h).ok_or_else(|| RenderError::Other(format!("cannot allocate {w}×{h} canvas")))
}

/// Write premultiplied gray `g` with alpha `a` into pixel `i`.
fn put(pm: &mut Pixmap, i: usize, g: u8, a: u8) {
    let pg = ((u16::from(g) * u16::from(a) + 127) / 255) as u8;
    if let Some(px) = pm.data_mut().get_mut(i * 4..i * 4 + 4) {
        px.copy_from_slice(&[pg, pg, pg, a]);
    }
}

struct Ctx<'a> {
    doc: &'a Document,
    opts: &'a RenderOptions,
    dpi: u32,
    issues: Vec<RenderIssue>,
}

impl Ctx<'_> {
    fn subst(&self, s: &str) -> String {
        text::substitute(s, |name| {
            self.opts
                .values
                .get(name)
                .or_else(|| self.doc.variables.get(name))
                .map(String::as_str)
        })
    }

    fn px(&self, pt: f64) -> f32 {
        pt_to_dots(pt, self.dpi) as f32
    }
}

fn render_page(
    engine: &mut TextEngine,
    doc: &Document,
    opts: &RenderOptions,
) -> Result<(RenderOutput, GrayImage), RenderError> {
    let page = doc
        .pages
        .get(opts.page)
        .ok_or(RenderError::NoSuchPage(opts.page))?;
    let (wmm, hmm) = (doc.label.width_mm, doc.label.height_mm);
    let bad = || RenderError::BadSize {
        width_mm: wmm,
        height_mm: hmm,
    };
    if opts.dpi == 0 || !wmm.is_finite() || !hmm.is_finite() {
        return Err(bad());
    }
    let (lw, lh) = (mm_to_dots(wmm, opts.dpi), mm_to_dots(hmm, opts.dpi));
    if !(1..=MAX_DOTS).contains(&lw) || !(1..=MAX_DOTS).contains(&lh) {
        return Err(bad());
    }
    let (lw, lh) = (lw as u32, lh as u32);
    let mut ctx = Ctx {
        doc,
        opts,
        dpi: opts.dpi,
        issues: Vec::new(),
    };
    let mut bitmap = Bitmap1::new(lw, lh);
    let mut gray = vec![255u8; (lw * lh) as usize];
    for el in page.elements.iter().filter(|e| !e.hidden) {
        if let Some((layer, conv)) = render_element(&mut ctx, engine, el, lw, lh)? {
            let (covered, black) = compose(&mut bitmap, &mut gray, &layer, conv);
            if matches!(el.kind, ElementKind::Image(_)) && covered > 0 {
                let pct = black as f64 * 100.0 / covered as f64;
                if !(LOW_CONTRAST_PCT..=100.0 - LOW_CONTRAST_PCT).contains(&pct) {
                    ctx.issues.push(RenderIssue::LowContrast {
                        element_id: el.id,
                        black_percent: (pct * 10.0).round() / 10.0,
                    });
                }
            }
        }
    }
    let out = RenderOutput {
        bitmap,
        issues: ctx.issues,
    };
    Ok((
        out,
        GrayImage {
            width: lw,
            height: lh,
            data: gray,
        },
    ))
}

/// Images with less than this share (in %) of black or white dots are reported as low contrast.
const LOW_CONTRAST_PCT: f64 = 2.0;

/// Source-over the layer onto the gray preview and its 1-bit conversion onto the bitmap.
/// Returns (covered dots, black dots among them).
fn compose(bitmap: &mut Bitmap1, gray: &mut [u8], layer: &Pixmap, conv: Conv) -> (u64, u64) {
    let (w, h) = (layer.width(), layer.height());
    let data = layer.data();
    let n = (w * h) as usize;
    let mut covered = vec![false; n];
    let mut value = vec![255u8; n];
    for i in 0..n {
        let (pg, a) = (data[i * 4], data[i * 4 + 3]);
        if a == 0 {
            continue;
        }
        let dst = u16::from(gray[i]);
        gray[i] = (u16::from(pg) + (dst * (255 - u16::from(a)) + 127) / 255).min(255) as u8;
        if a >= 128 {
            covered[i] = true;
            value[i] = ((u16::from(pg) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8;
        }
    }
    let black = match conv {
        Conv::Threshold => value.iter().map(|v| *v < 128).collect(),
        Conv::Dither(m, t) => {
            dither::dither_masked(&value, Some(&covered), w as usize, h as usize, m, t)
        }
    };
    let (mut n_cov, mut n_black) = (0u64, 0u64);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if covered[i] {
                bitmap.set(x, y, black[i]);
                n_cov += 1;
                n_black += u64::from(black[i]);
            }
        }
    }
    (n_cov, n_black)
}

fn render_element(
    ctx: &mut Ctx<'_>,
    engine: &mut TextEngine,
    el: &Element,
    lw: u32,
    lh: u32,
) -> Result<Option<(Pixmap, Conv)>, RenderError> {
    let dpi = ctx.dpi;
    let dpm = dpi as f32 / 25.4;
    let x0 = mm_to_dots(el.x_mm, dpi);
    let y0 = mm_to_dots(el.y_mm, dpi);
    let fw = mm_to_dots(el.x_mm + el.width_mm, dpi) - x0;
    let fh = mm_to_dots(el.y_mm + el.height_mm, dpi) - y0;
    let rot = if el.rotation_deg.is_finite() {
        el.rotation_deg as f32
    } else {
        0.0
    };
    let (sx, sy) = (
        if el.flip_h { -1.0 } else { 1.0 },
        if el.flip_v { -1.0 } else { 1.0 },
    );
    let ts = |w: f32, h: f32| {
        let mut t = Transform::from_translate(x0 as f32, y0 as f32);
        if rot != 0.0 {
            t = t.pre_rotate_at(rot, w / 2.0, h / 2.0);
        }
        if sx < 0.0 || sy < 0.0 {
            t = t
                .pre_translate(w / 2.0, h / 2.0)
                .pre_scale(sx, sy)
                .pre_translate(-w / 2.0, -h / 2.0);
        }
        t
    };
    let mut layer = pixmap(lw, lh)?;
    if let ElementKind::Line(p) = &el.kind {
        shapes::draw_line(
            &mut layer,
            p,
            fw as f32,
            fh as f32,
            dpm,
            ts(fw as f32, fh as f32),
        );
        return Ok(Some((layer, Conv::Threshold)));
    }
    if fw <= 0 || fh <= 0 || fw > MAX_DOTS || fh > MAX_DOTS {
        return Ok(None);
    }
    let (w, h) = (fw as u32, fh as u32);
    let frame_ts = ts(w as f32, h as f32);
    let local = match &el.kind {
        ElementKind::Rect(p) | ElementKind::Ellipse(p) => {
            let ellipse = matches!(el.kind, ElementKind::Ellipse(_));
            shapes::draw_shape(&mut layer, p, ellipse, w as f32, h as f32, dpm, frame_ts);
            return Ok(Some((layer, Conv::Threshold)));
        }
        ElementKind::Line(_) => return Ok(None),
        ElementKind::Border(p) => {
            shapes::draw_border(&mut layer, p, w as f32, h as f32, dpm, frame_ts);
            return Ok(Some((layer, Conv::Threshold)));
        }
        ElementKind::Table(p) => Some((table_layer(ctx, engine, el, p, w, h)?, Conv::Threshold)),
        ElementKind::Icon(p) => match icons::render(&p.icon, w, h, p.keep_aspect) {
            Some(mut pm) => {
                if p.invert {
                    invert_alpha(&mut pm);
                }
                Some((pm, Conv::Threshold))
            }
            None => {
                ctx.issues.push(RenderIssue::BadAsset {
                    element_id: el.id,
                    message: format!("unknown icon `{}`", p.icon),
                });
                None
            }
        },
        ElementKind::Text(p) => Some((text_layer(ctx, engine, el, p, w, h)?, Conv::Threshold)),
        ElementKind::Barcode(p) => {
            barcode_layer(ctx, engine, el, p, w, h)?.map(|l| (l, Conv::Threshold))
        }
        ElementKind::Code2d(p) => code2d_layer(ctx, el, p, w, h)?.map(|l| (l, Conv::Threshold)),
        ElementKind::Image(p) => {
            image_layer(ctx, el, p, w, h)?.map(|l| (l, Conv::Dither(p.dither, p.threshold)))
        }
    };
    let Some((local, conv)) = local else {
        return Ok(None);
    };
    let paint = PixmapPaint {
        quality: FilterQuality::Nearest,
        ..PixmapPaint::default()
    };
    layer.draw_pixmap(0, 0, local.as_ref(), &paint, frame_ts, None);
    Ok(Some((layer, conv)))
}

fn text_style(engine: &TextEngine, ctx: &mut Ctx<'_>, el: &Element, p: &TextProps) -> TextStyle {
    let family = engine.find_family(&p.font_family).unwrap_or_else(|| {
        ctx.issues.push(RenderIssue::FontFallback {
            element_id: el.id,
            requested: p.font_family.clone(),
        });
        DEFAULT_FONT.to_owned()
    });
    TextStyle {
        family,
        size_px: ctx.px(p.font_size_pt),
        line_spacing: p.line_spacing as f32,
        letter_spacing_px: (p.letter_spacing_mm * f64::from(ctx.dpi) / 25.4) as f32,
        bold: p.bold,
        italic: p.italic,
        underline: p.underline,
        align: p.align,
        wrap: p.wrap,
    }
}

fn text_layer(
    ctx: &mut Ctx<'_>,
    engine: &mut TextEngine,
    el: &Element,
    p: &TextProps,
    w: u32,
    h: u32,
) -> Result<Pixmap, RenderError> {
    let s = ctx.subst(&p.text);
    let s = match p.direction {
        TextDirection::Horizontal => s,
        TextDirection::Vertical => text::vertical(&s),
    };
    draw_text(ctx, engine, el, p, &s, w, h)
}

/// Lay out and draw already-substituted text `s` with `p`'s style into a `w`×`h` pixmap.
fn draw_text(
    ctx: &mut Ctx<'_>,
    engine: &mut TextEngine,
    el: &Element,
    p: &TextProps,
    s: &str,
    w: u32,
    h: u32,
) -> Result<Pixmap, RenderError> {
    let s = s.to_owned();
    let mut st = text_style(engine, ctx, el, p);
    let (bw, bh) = (w as f32, h as f32);
    let fits = |l: &text::Laid| l.width <= bw + 0.5 && l.height <= bh + 0.5;
    let mut laid = engine.layout(&s, &st, bw);
    if !fits(&laid) && p.fit == TextFit::Shrink && p.font_size_pt > 1.0 {
        // Largest size in 0.1 pt steps (down to 1 pt) that fits.
        let (mut lo, mut hi) = (10i64, (p.font_size_pt * 10.0).floor() as i64 - 1);
        let mut best = None;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            st.size_px = ctx.px(mid as f64 / 10.0);
            if fits(&engine.layout(&s, &st, bw)) {
                best = Some(mid);
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        let size = best.unwrap_or(10) as f64 / 10.0;
        st.size_px = ctx.px(size);
        laid = engine.layout(&s, &st, bw);
        ctx.issues.push(RenderIssue::TextShrunk {
            element_id: el.id,
            font_size_pt: size,
        });
    }
    if !fits(&laid) {
        ctx.issues
            .push(RenderIssue::TextOverflow { element_id: el.id });
    }
    let dy = match p.valign {
        VAlign::Top => 0.0,
        VAlign::Middle => (bh - laid.height) / 2.0,
        VAlign::Bottom => bh - laid.height,
    }
    .round() as i32;
    let mut cov = vec![0u8; (w * h) as usize];
    engine.draw(&mut laid, 0, dy, &mut cov, w, h);
    let mut pm = pixmap(w, h)?;
    for (i, c) in cov.iter().enumerate() {
        if p.invert {
            put(&mut pm, i, *c, 255);
        } else {
            put(&mut pm, i, 0, *c);
        }
    }
    Ok(pm)
}

/// Opaque black where the pixmap is transparent and transparent where it is inked.
fn invert_alpha(pm: &mut Pixmap) {
    for px in pm.data_mut().chunks_mut(4) {
        let a = 255 - px[3];
        px.copy_from_slice(&[0, 0, 0, a]);
    }
}

/// Cumulative edges in dots for relative `weights` spread over `total` dots.
fn edges(weights: &[f64], total: u32) -> Vec<u32> {
    let sum: f64 = weights.iter().sum::<f64>().max(f64::MIN_POSITIVE);
    let mut acc = 0.0;
    let mut out = vec![0];
    for w in weights {
        acc += w;
        out.push(((acc / sum) * f64::from(total)).round() as u32);
    }
    out
}

/// A table: grid lines (skipping lines inside merges) and shrink-to-fit cell text.
fn table_layer(
    ctx: &mut Ctx<'_>,
    engine: &mut TextEngine,
    el: &Element,
    p: &TableProps,
    w: u32,
    h: u32,
) -> Result<Pixmap, RenderError> {
    let mut pm = pixmap(w, h)?;
    if p.validate().is_err() {
        ctx.issues.push(RenderIssue::BadAsset {
            element_id: el.id,
            message: "the table's cells, sizes or merges are inconsistent".to_owned(),
        });
        return Ok(pm);
    }
    let dpm = ctx.dpi as f32 / 25.4;
    let s = if p.stroke_mm > 0.0 {
        (p.stroke_mm as f32 * dpm).round().max(1.0)
    } else {
        0.0
    };
    // Lines are centred on the edges but kept inside the frame.
    let si = s as u32;
    let xs = edges(&p.column_widths, w.saturating_sub(si));
    let ys = edges(&p.row_heights, h.saturating_sub(si));
    let (rows, cols) = (p.row_heights.len(), p.column_widths.len());
    let same = |r1: usize, c1: usize, r2: usize, c2: usize| match p.merge_at(r1, c1) {
        Some(m) => m.contains(r2, c2),
        None => false,
    };
    let pad = (0.4 * dpm).round() as u32 + si;
    for r in 0..rows {
        for c in 0..cols {
            if p.merge_at(r, c).is_some_and(|m| (m.row, m.col) != (r, c)) {
                continue;
            }
            let (r2, c2) = p
                .merge_at(r, c)
                .map_or((r + 1, c + 1), |m| (m.row + m.rows, m.col + m.cols));
            let (x0, y0, x1, y1) = (xs[c], ys[r], xs[c2], ys[r2]);
            let cell = &p.cells[r][c];
            if cell.invert {
                fill_black(&mut pm, x0, y0, x1 - x0 + si, y1 - y0 + si);
            }
            let text = ctx.subst(&cell.text);
            if text.is_empty() {
                continue;
            }
            let (cw, ch) = (
                (x1 - x0).saturating_sub(pad + si / 2),
                (y1 - y0).saturating_sub(si),
            );
            if cw < 2 || ch < 2 {
                continue;
            }
            let tp = TextProps {
                text: String::new(),
                font_family: p.font_family.clone(),
                font_size_pt: cell.font_size_pt.unwrap_or(p.font_size_pt),
                bold: cell.bold,
                align: cell.align.unwrap_or(p.align),
                valign: VAlign::Middle,
                fit: TextFit::Shrink,
                invert: false,
                ..Default::default()
            };
            let layer = draw_text(ctx, engine, el, &tp, &text, cw, ch)?;
            let paint = PixmapPaint::default();
            if cell.invert {
                // White text: punch the glyphs out of the black cell.
                let mut white = layer;
                for px in white.data_mut().chunks_mut(4) {
                    let a = px[3];
                    px.copy_from_slice(&[a, a, a, a]);
                }
                pm.draw_pixmap(
                    (x0 + pad / 2 + si / 2) as i32,
                    (y0 + si) as i32,
                    white.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                );
            } else {
                pm.draw_pixmap(
                    (x0 + pad / 2 + si / 2) as i32,
                    (y0 + si) as i32,
                    layer.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
    if s > 0.0 {
        let id = Transform::identity();
        let sf = s;
        // Vertical lines, row by row so merged cells stay open.
        for (c, &x) in xs.iter().enumerate() {
            for r in 0..rows {
                let inner = c > 0 && c < cols && same(r, c - 1, r, c);
                if !inner {
                    shapes::bar(
                        &mut pm,
                        x as f32,
                        ys[r] as f32,
                        x as f32 + sf,
                        ys[r + 1] as f32 + sf,
                        id,
                    );
                }
            }
        }
        for (r, &y) in ys.iter().enumerate() {
            for c in 0..cols {
                let inner = r > 0 && r < rows && same(r - 1, c, r, c);
                if !inner {
                    shapes::bar(
                        &mut pm,
                        xs[c] as f32,
                        y as f32,
                        xs[c + 1] as f32 + sf,
                        y as f32 + sf,
                        id,
                    );
                }
            }
        }
    }
    Ok(pm)
}

/// White opaque pixmap.
fn white(w: u32, h: u32) -> Result<Pixmap, RenderError> {
    let mut pm = pixmap(w, h)?;
    pm.fill(tiny_skia::Color::WHITE);
    Ok(pm)
}

fn fill_black(pm: &mut Pixmap, x: u32, y: u32, w: u32, h: u32) {
    let pw = pm.width();
    for yy in y..(y + h).min(pm.height()) {
        for xx in x..(x + w).min(pw) {
            put(pm, (yy * pw + xx) as usize, 0, 255);
        }
    }
}

fn too_small(ctx: &mut Ctx<'_>, el: &Element, modules: u32, avail: u32) {
    ctx.issues.push(RenderIssue::ModuleWidth {
        element_id: el.id,
        dots: f64::from(avail) / f64::from(modules.max(1)),
    });
    ctx.issues.push(RenderIssue::EncodeFailed {
        element_id: el.id,
        message: format!("box too small: {modules} modules need at least {modules} dots"),
    });
}

fn barcode_layer(
    ctx: &mut Ctx<'_>,
    engine: &mut TextEngine,
    el: &Element,
    p: &BarcodeProps,
    w: u32,
    h: u32,
) -> Result<Option<Pixmap>, RenderError> {
    let data = ctx.subst(&p.data);
    let modules = match codes::encode_1d(&data, p.symbology) {
        Ok(m) => m,
        Err(message) => {
            ctx.issues.push(RenderIssue::EncodeFailed {
                element_id: el.id,
                message,
            });
            return Ok(None);
        }
    };
    let n = modules.len() as u32;
    let text_px = ctx.px(p.text_size_pt);
    let th = match p.human_readable {
        HumanReadable::None => 0,
        _ => (text_px * 1.25).ceil().max(0.0) as u32,
    };
    let bar_h = h.saturating_sub(th);
    let module = w / n.max(1);
    if module == 0 || bar_h == 0 {
        too_small(ctx, el, n, w);
        return Ok(None);
    }
    ctx.issues.push(RenderIssue::ModuleWidth {
        element_id: el.id,
        dots: f64::from(module),
    });
    let mut pm = white(w, h)?;
    let x0 = (w - n * module) / 2;
    ctx.issues.push(RenderIssue::CodeLayout {
        element_id: el.id,
        module_dots: module,
        slack_dots: x0,
        quiet_modules: 10,
    });
    let bar_y = if p.human_readable == HumanReadable::Above {
        th
    } else {
        0
    };
    for (i, dark) in modules.iter().enumerate() {
        if *dark {
            fill_black(&mut pm, x0 + i as u32 * module, bar_y, module, bar_h);
        }
    }
    if th > 0 {
        let st = TextStyle {
            family: DEFAULT_FONT.to_owned(),
            size_px: text_px,
            line_spacing: 1.0,
            letter_spacing_px: 0.0,
            bold: false,
            italic: false,
            underline: false,
            align: HAlign::Center,
            wrap: false,
        };
        let mut laid = engine.layout(&data, &st, w as f32);
        let text_y = if p.human_readable == HumanReadable::Above {
            0
        } else {
            bar_h
        };
        let dy = text_y as i32 + ((th as f32 - laid.height) / 2.0).round() as i32;
        let mut cov = vec![0u8; (w * h) as usize];
        engine.draw(&mut laid, 0, dy, &mut cov, w, h);
        for (i, c) in cov.iter().enumerate() {
            if *c > 0 {
                let base = pm.data()[i * 4];
                put(&mut pm, i, base.min(255 - *c), 255);
            }
        }
    }
    Ok(Some(pm))
}

fn code2d_layer(
    ctx: &mut Ctx<'_>,
    el: &Element,
    p: &Code2dProps,
    w: u32,
    h: u32,
) -> Result<Option<Pixmap>, RenderError> {
    let data = ctx.subst(&p.data);
    let m = match codes::encode_2d(&data, p.symbology, p.ecc) {
        Ok(m) => m,
        Err(message) => {
            ctx.issues.push(RenderIssue::EncodeFailed {
                element_id: el.id,
                message,
            });
            return Ok(None);
        }
    };
    let module = (w / m.width).min(h / m.height);
    if module == 0 {
        let (modules, avail) = if w * m.height < h * m.width {
            (m.width, w)
        } else {
            (m.height, h)
        };
        too_small(ctx, el, modules, avail);
        return Ok(None);
    }
    ctx.issues.push(RenderIssue::ModuleWidth {
        element_id: el.id,
        dots: f64::from(module),
    });
    let mut pm = white(w, h)?;
    let x0 = (w - m.width * module) / 2;
    let y0 = (h - m.height * module) / 2;
    ctx.issues.push(RenderIssue::CodeLayout {
        element_id: el.id,
        module_dots: module,
        slack_dots: x0.min(y0),
        quiet_modules: match p.symbology {
            Symbology2d::Qr => 4,
            _ => 1,
        },
    });
    for y in 0..m.height {
        for x in 0..m.width {
            if m.get(x, y) {
                fill_black(&mut pm, x0 + x * module, y0 + y * module, module, module);
            }
        }
    }
    Ok(Some(pm))
}

fn image_layer(
    ctx: &mut Ctx<'_>,
    el: &Element,
    p: &ImageProps,
    w: u32,
    h: u32,
) -> Result<Option<Pixmap>, RenderError> {
    // The key may be a `{{column}}` binding filled per record (data.attach_images).
    let key = if p.asset.contains("{{") {
        ctx.subst(&p.asset)
    } else {
        p.asset.clone()
    };
    let img = match load_image(ctx.doc, &key) {
        Ok(img) => img,
        Err(message) => {
            ctx.issues.push(RenderIssue::BadAsset {
                element_id: el.id,
                message,
            });
            return Ok(None);
        }
    };
    let (iw, ih) = (f64::from(img.width()), f64::from(img.height()));
    let (fw, fh) = (f64::from(w), f64::from(h));
    let (dw, dh) = match p.fit {
        ImageFit::Stretch => (fw, fh),
        ImageFit::Contain => {
            let s = (fw / iw).min(fh / ih);
            (iw * s, ih * s)
        }
        ImageFit::Cover => {
            let s = (fw / iw).max(fh / ih);
            (iw * s, ih * s)
        }
    };
    let (dw, dh) = (
        dw.round().clamp(1.0, 65_536.0) as u32,
        dh.round().clamp(1.0, 65_536.0) as u32,
    );
    let scaled = image::imageops::resize(&img, dw, dh, image::imageops::FilterType::Triangle);
    let ox = (i64::from(w) - i64::from(dw)) / 2;
    let oy = (i64::from(h) - i64::from(dh)) / 2;
    let lut = dither::adjust_lut(p.brightness, p.contrast, p.invert);
    let mut pm = pixmap(w, h)?;
    for y in 0..h {
        let sy = i64::from(y) - oy;
        if sy < 0 || sy >= i64::from(dh) {
            continue;
        }
        for x in 0..w {
            let sx = i64::from(x) - ox;
            if sx < 0 || sx >= i64::from(dw) {
                continue;
            }
            let [r, g, b, a] = scaled.get_pixel(sx as u32, sy as u32).0;
            let luma = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 + 500) / 1000;
            put(
                &mut pm,
                (y * w + x) as usize,
                lut[luma.min(255) as usize],
                a,
            );
        }
    }
    Ok(Some(pm))
}

fn load_image(doc: &Document, key: &str) -> Result<image::RgbaImage, String> {
    let asset = doc
        .assets
        .get(key)
        .ok_or_else(|| format!("asset {key} not found"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(asset.data_base64.trim())
        .map_err(|e| format!("invalid base64: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("cannot decode image: {e}"))?;
    if img.width() == 0 || img.height() == 0 {
        return Err("empty image".to_owned());
    }
    Ok(img.to_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_to_dots_rounds() {
        assert_eq!(mm_to_dots(50.0, 300), 591);
        assert_eq!(mm_to_dots(30.0, 300), 354);
        assert_eq!(mm_to_dots(50.0, 203), 400);
        assert_eq!(mm_to_dots(25.4, 203), 203);
        assert_eq!(mm_to_dots(0.0, 300), 0);
        assert!((pt_to_dots(72.0, 300) - 300.0).abs() < 1e-9);
    }

    #[test]
    fn png_is_one_bit() {
        let mut b = Bitmap1::new(9, 2);
        b.set(0, 0, true);
        let png = bitmap_to_png(&b).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let info = dec.info();
        assert_eq!((info.width, info.height), (9, 2));
        assert_eq!(info.bit_depth, png::BitDepth::One);
    }
}
