//! Template gallery and "save as template" dialogs (ADR-0018). Every library change runs a
//! `template.*` command; the gallery only caches the listing and thumbnails.

use egui::{CornerRadius, Margin, RichText, Stroke, Ui, vec2};
use labely_core::commands::template::{self, Category, TemplateInfo};
use labely_core::services::RenderOptions;
use serde_json::json;

use crate::app::LabelyApp;
use crate::{icons, theme};

/// Which group the gallery shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shelf {
    Starters,
    /// A library folder; empty is the default folder.
    Folder(String),
}

/// Template library page state.
#[derive(Debug, Clone, PartialEq)]
pub struct Gallery {
    pub shelf: Shelf,
    pub query: String,
    /// Selected template reference.
    pub selected: Option<String>,
    pub templates: Vec<TemplateInfo>,
    pub folders: Vec<String>,
    /// The listing must be read again.
    pub stale: bool,
    /// Loaded roll the listing was read for (its `fits` refer to it).
    pub paper: Option<String>,
    /// Starter category filter; `None` shows every category.
    pub category: Option<Category>,
    /// Text of the rename field (template or folder).
    pub rename: String,
    pub new_folder: String,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            shelf: Shelf::Starters,
            query: String::new(),
            selected: None,
            templates: Vec::new(),
            folders: Vec::new(),
            stale: true,
            paper: None,
            category: None,
            rename: String::new(),
            new_folder: String::new(),
        }
    }
}

/// "Save as template" form.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SaveForm {
    pub name: String,
    pub folder: String,
    pub folders: Vec<String>,
    /// A template of that name exists; the next save replaces it.
    pub exists: bool,
}

impl SaveForm {
    pub fn new(app: &mut LabelyApp) -> Self {
        let name = app
            .session
            .doc(None)
            .map(|d| d.name.clone())
            .unwrap_or_default();
        let folders = list(app).map(|(_, f)| f).unwrap_or_default();
        Self {
            name,
            folders,
            ..Self::default()
        }
    }
}

/// Localised display name: starters have a `template-<id>` key.
pub fn display_name(app: &LabelyApp, t: &TemplateInfo) -> String {
    if let Some(id) = t.reference.strip_prefix("starter:") {
        let key = format!("template-{id}");
        if app.i18n.has(&key) {
            return app.i18n.t(&key);
        }
    }
    t.name.clone()
}

/// The starter categories, in gallery order.
pub const CATEGORIES: [Category; 5] = [
    Category::Kitchen,
    Category::Office,
    Category::Cables,
    Category::Shipping,
    Category::Events,
];

/// Localised category name.
pub fn category_name(app: &LabelyApp, c: Category) -> String {
    let id = match c {
        Category::Kitchen => "kitchen",
        Category::Office => "office",
        Category::Cables => "cables",
        Category::Shipping => "shipping",
        Category::Events => "events",
    };
    app.i18n.t(&format!("template-category-{id}"))
}

/// Paper id of the loaded roll, when the printer's RFID tag was recognised (`paper.detect`).
pub(crate) fn loaded_paper(app: &LabelyApp) -> Option<String> {
    app.ui.detected.as_ref()?["paper"]["id"]
        .as_str()
        .map(str::to_owned)
}

/// `template.list` for the loaded roll: templates that fit it come first, with `fits` set.
pub(crate) fn list(app: &mut LabelyApp) -> Option<(Vec<TemplateInfo>, Vec<String>)> {
    let mut params = json!({});
    if let Some(id) = loaded_paper(app) {
        params["paper"] = json!(id);
    }
    let env = app.run_reporting("template.list", params);
    let r = env.result?;
    let templates = serde_json::from_value(r["templates"].clone()).ok()?;
    let folders = serde_json::from_value(r["folders"].clone()).unwrap_or_default();
    Some((templates, folders))
}

/// Grey thumbnail of a template, cached by reference.
pub(crate) fn thumbnail(
    app: &mut LabelyApp,
    ctx: &egui::Context,
    t: &TemplateInfo,
) -> Option<egui::TextureHandle> {
    if let Some(cached) = app.thumbs.get(&t.reference) {
        return cached.clone();
    }
    let texture = (|| {
        let doc = template::load(&app.session, &t.reference).ok()?;
        let dpi = (110.0 * 25.4 / doc.label.width_mm.max(doc.label.height_mm)).clamp(40.0, 203.0);
        let row = doc.data.as_ref().map(|_| 0);
        let opts = RenderOptions {
            dpi: dpi as u32,
            page: 0,
            values: labely_core::commands::binding_values(&doc, row, 0),
        };
        let out = app.session.services().renderer.render(&doc, &opts).ok()?;
        let (w, h) = (out.bitmap.width(), out.bitmap.height());
        let mut gray = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                gray.push(if out.bitmap.get(x, y) { 0u8 } else { 255 });
            }
        }
        let image = egui::ColorImage::from_gray([w as usize, h as usize], &gray);
        Some(ctx.load_texture(
            format!("thumb-{}", t.reference),
            image,
            egui::TextureOptions::LINEAR,
        ))
    })();
    app.thumbs.insert(t.reference.clone(), texture.clone());
    texture
}

/// Show the template library page of the home screen.
pub fn open_library(app: &mut LabelyApp) {
    app.ui.gallery.stale = true;
    app.ui.home_page = crate::home::Page::Templates;
    app.ui.home = true;
}

fn mm(v: f64) -> String {
    crate::home::mm(v)
}

/// The template library page (design screen 7): shelves, cards, the selected template.
pub fn library_page(app: &mut LabelyApp, ui: &mut Ui) {
    let mut g = std::mem::take(&mut app.ui.gallery);
    let paper = loaded_paper(app);
    if g.stale || g.paper != paper {
        g.paper = paper;
        if let Some((templates, folders)) = list(app) {
            g.templates = templates;
            g.folders = folders;
        }
        app.thumbs.clear();
        app.ui.template_cache = None;
        g.stale = false;
    }
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(app.i18n.t("templates-title"))
                    .font(theme::bold(25.0))
                    .color(theme::pal().text),
            );
            ui.label(RichText::new(app.i18n.t("templates-subtitle")).color(theme::pal().muted));
        });
        theme::trailing(ui, |ui| {
            let has_doc = app.session.active_doc_id().is_some();
            ui.add_enabled_ui(has_doc, |ui| {
                let text = crate::menu::command_label(&app.i18n, "template.save");
                if icons::text_button(ui, "plus", &text, None, theme::pal().text, None).clicked() {
                    crate::menu::invoke(app, "template.save");
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut g.query)
                    .hint_text(app.i18n.t("templates-search"))
                    .desired_width(260.0)
                    .min_size(vec2(0.0, 30.0)),
            );
        });
    });
    ui.add_space(14.0);
    let h = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        let side = 210.0;
        let detail = 280.0;
        let mid = (ui.available_width() - side - detail - 32.0).max(220.0);
        ui.allocate_ui_with_layout(
            vec2(side, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                theme::card().inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(side - 22.0);
                    ui.set_min_height(h - 24.0);
                    shelves(app, ui, &mut g);
                });
            },
        );
        ui.allocate_ui_with_layout(
            vec2(mid, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(mid);
                cards(app, ui, &mut g, h);
            },
        );
        ui.allocate_ui_with_layout(
            vec2(detail, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                theme::card().show(ui, |ui| {
                    ui.set_width(detail - 30.0);
                    ui.set_min_height(h - 30.0);
                    details(app, ui, &mut g);
                });
            },
        );
    });
    app.ui.gallery = g;
}

/// A shelf row: icon, name, count; selected rows are tinted.
fn shelf_row(ui: &mut Ui, icon: &str, text: &str, count: usize, active: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), egui::Sense::click());
    let label = text.to_owned();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, &label));
    let p = ui.painter();
    if active {
        p.rect_filled(
            rect,
            CornerRadius::same(theme::RADIUS),
            theme::pal().primary_tint,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(theme::RADIUS), theme::pal().bg);
    }
    let fg = if active {
        theme::pal().primary
    } else {
        theme::pal().text
    };
    let ir = egui::Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0));
    icons::paint(p, ir, icon, fg, 1.5);
    let font = if active {
        theme::bold(13.0)
    } else {
        theme::regular(13.0)
    };
    p.text(
        rect.left_center() + vec2(36.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        fg,
    );
    p.text(
        rect.right_center() - vec2(10.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        theme::regular(11.5),
        theme::pal().muted,
    );
    resp
}

fn shelves(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) {
    let count = |g: &Gallery, shelf: &Shelf| {
        g.templates
            .iter()
            .filter(|t| match shelf {
                Shelf::Starters => t.starter,
                Shelf::Folder(f) => !t.starter && &t.folder == f,
            })
            .count()
    };
    let pick = |ui: &mut Ui, shelf: Shelf, icon: &str, text: String, g: &mut Gallery| {
        let n = count(g, &shelf);
        if shelf_row(ui, icon, &text, n, g.shelf == shelf).clicked() && g.shelf != shelf {
            if let Shelf::Folder(f) = &shelf {
                g.rename = f.clone();
            }
            g.shelf = shelf;
            g.selected = None;
        }
    };
    pick(
        ui,
        Shelf::Starters,
        "sparkle",
        app.i18n.t("templates-starters"),
        g,
    );
    theme::section(ui, &app.i18n.t("templates-mine"));
    pick(
        ui,
        Shelf::Folder(String::new()),
        "folder",
        app.i18n.t("templates-default-folder"),
        g,
    );
    for f in g.folders.clone() {
        pick(ui, Shelf::Folder(f.clone()), "folder", f, g);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let w = ui.available_width() - 36.0;
        ui.add(
            egui::TextEdit::singleline(&mut g.new_folder)
                .hint_text(app.i18n.t("templates-new-folder"))
                .desired_width(w),
        );
        let ok = !g.new_folder.trim().is_empty();
        if ui
            .add_enabled(
                ok,
                egui::Button::new(RichText::new("+").color(theme::pal().primary)),
            )
            .on_hover_text(app.i18n.t("templates-new-folder"))
            .clicked()
        {
            let name = g.new_folder.trim().to_owned();
            if app
                .run_reporting("template.folder", json!({"op": "create", "folder": name}))
                .ok
            {
                g.shelf = Shelf::Folder(name.clone());
                g.rename = name;
                g.new_folder.clear();
                g.stale = true;
            }
        }
    });
    if let Shelf::Folder(f) = g.shelf.clone()
        && !f.is_empty()
        && g.selected.is_none()
    {
        ui.add_space(8.0);
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut g.rename)
                .hint_text(app.i18n.t("templates-folder"))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button(app.i18n.t("templates-rename-folder")).clicked()
                && app
                    .run_reporting(
                        "template.folder",
                        json!({"op": "rename", "folder": f, "to": g.rename.trim()}),
                    )
                    .ok
            {
                g.shelf = Shelf::Folder(g.rename.trim().to_owned());
                g.stale = true;
            }
            if ui
                .button(
                    RichText::new(app.i18n.t("templates-delete-folder")).color(theme::pal().red),
                )
                .clicked()
                && app
                    .run_reporting("template.folder", json!({"op": "delete", "folder": f}))
                    .ok
            {
                g.shelf = Shelf::Folder(String::new());
                g.stale = true;
            }
        });
    }
}

/// The visible templates of the current shelf, filtered by the search text.
fn visible(app: &LabelyApp, g: &Gallery) -> Vec<TemplateInfo> {
    let q = g.query.trim().to_lowercase();
    g.templates
        .iter()
        .filter(|t| match &g.shelf {
            Shelf::Starters => t.starter && (g.category.is_none() || t.category == g.category),
            Shelf::Folder(f) => !t.starter && &t.folder == f,
        })
        .filter(|t| q.is_empty() || display_name(app, t).to_lowercase().contains(&q))
        .cloned()
        .collect()
}

fn cards(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery, height: f32) {
    let items = visible(app, g);
    let shelf = match &g.shelf {
        Shelf::Starters => app.i18n.t("templates-starters"),
        Shelf::Folder(f) if f.is_empty() => app.i18n.t("templates-default-folder"),
        Shelf::Folder(f) => f.clone(),
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(shelf)
                .font(theme::bold(15.0))
                .color(theme::pal().text),
        );
        ui.label(
            RichText::new(app.i18n.t_args(
                "templates-count",
                &[("count", labely_i18n::Arg::Num(items.len() as f64))],
            ))
            .size(12.0)
            .color(theme::pal().muted),
        );
    });
    if g.shelf == Shelf::Starters {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let all = app.i18n.t("templates-category-all");
            if crate::home::chip(ui, &all, g.category.is_none()).clicked() {
                g.category = None;
            }
            for c in CATEGORIES {
                let active = g.category == Some(c);
                if crate::home::chip(ui, &category_name(app, c), active).clicked() {
                    g.category = Some(c);
                }
            }
        });
    }
    ui.add_space(8.0);
    if items.is_empty() {
        ui.label(RichText::new(app.i18n.t("templates-empty")).color(theme::pal().muted));
        return;
    }
    let ctx = ui.ctx().clone();
    let mut open = None;
    egui::ScrollArea::vertical()
        .id_salt("template-cards")
        .max_height(height - 40.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let gap = 12.0;
            let per_row = ((ui.available_width() + gap) / 190.0).floor().max(1.0);
            let w = (ui.available_width() - gap * (per_row - 1.0)) / per_row;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(gap, gap);
                for t in &items {
                    let selected = g.selected.as_deref() == Some(t.reference.as_str());
                    let name = display_name(app, t);
                    let thumb = thumbnail(app, &ctx, t);
                    let size = format!("{} × {} mm", mm(t.width_mm), mm(t.height_mm));
                    let fits = (t.fits == Some(true)).then(|| app.i18n.t("templates-fits"));
                    let resp = card(
                        ui,
                        &name,
                        &size,
                        fits.as_deref(),
                        thumb.as_ref(),
                        w,
                        selected,
                    );
                    if resp.clicked() {
                        g.selected = Some(t.reference.clone());
                        g.rename = name.clone();
                    }
                    if resp.double_clicked() {
                        open = Some(t.reference.clone());
                    }
                }
            });
        });
    if let Some(r) = open {
        open_template(app, &r);
    }
}

/// Template card: thumbnail on a grey well, name, size and the "fits the roll" note; named by
/// the template name.
fn card(
    ui: &mut Ui,
    name: &str,
    size: &str,
    fits: Option<&str>,
    thumb: Option<&egui::TextureHandle>,
    width: f32,
    selected: bool,
) -> egui::Response {
    let height = width * 0.55 + 56.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());
    let label = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, &label));
    if let Some(f) = fits {
        resp.clone().on_hover_text(f);
    }
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = ui.painter();
    let stroke = if selected {
        Stroke::new(2.0, theme::pal().primary)
    } else if resp.hovered() {
        Stroke::new(1.0, theme::pal().primary_tint2)
    } else {
        Stroke::new(1.0, theme::pal().border)
    };
    p.rect(
        rect,
        CornerRadius::same(theme::RADIUS_LG),
        theme::pal().surface,
        stroke,
        egui::StrokeKind::Inside,
    );
    let well = egui::Rect::from_min_size(
        rect.min + vec2(8.0, 8.0),
        vec2(width - 16.0, width * 0.55 - 8.0),
    );
    p.rect_filled(well, CornerRadius::same(theme::RADIUS), theme::pal().bg);
    if let Some(tex) = thumb {
        let s = tex.size_vec2();
        let k = ((well.width() - 24.0) / s.x)
            .min((well.height() - 20.0) / s.y)
            .min(1.4);
        let r = egui::Rect::from_center_size(well.center(), s * k);
        p.rect_filled(
            r.expand(1.0).translate(vec2(0.0, 1.5)),
            CornerRadius::same(3),
            egui::Color32::from_black_alpha(18),
        );
        p.image(
            tex.id(),
            r,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    p.text(
        egui::pos2(rect.left() + 12.0, well.bottom() + 18.0),
        egui::Align2::LEFT_CENTER,
        name,
        theme::bold(13.0),
        theme::pal().text,
    );
    let size_rect = p.text(
        egui::pos2(rect.left() + 12.0, well.bottom() + 36.0),
        egui::Align2::LEFT_CENTER,
        size,
        theme::regular(11.5),
        theme::pal().muted,
    );
    if let Some(f) = fits {
        fits_note(
            p,
            size_rect.right() + 8.0,
            well.bottom() + 36.0,
            rect.right() - 10.0,
            f,
        );
    }
    resp
}

/// "Fits the roll" note after a card's size, in the accent colour, cut to `right`.
pub(crate) fn fits_note(p: &egui::Painter, left: f32, y: f32, right: f32, text: &str) {
    theme::fit_text(
        p,
        egui::pos2(left, y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::bold(11.0),
        theme::pal().primary,
        (right - left).max(0.0),
    );
}

pub(crate) fn open_template(app: &mut LabelyApp, reference: &str) -> bool {
    let ok = app
        .run_reporting("template.new", json!({ "ref": reference }))
        .ok;
    if ok {
        app.ui.fit_pending = true;
        app.ui.record = 1;
    }
    ok
}

/// The selected template: preview, facts and actions.
fn details(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) {
    let Some(sel) = g.selected.clone() else {
        ui.label(RichText::new(app.i18n.t("templates-pick")).color(theme::pal().muted));
        return;
    };
    let Some(t) = g.templates.iter().find(|t| t.reference == sel).cloned() else {
        g.selected = None;
        return;
    };
    let starter = t.starter;
    let ctx = ui.ctx().clone();
    let thumb = thumbnail(app, &ctx, &t);
    let w = ui.available_width();
    let (well, _) = ui.allocate_exact_size(vec2(w, w * 0.62), egui::Sense::hover());
    ui.painter()
        .rect_filled(well, CornerRadius::same(theme::RADIUS), theme::pal().bg);
    if let Some(tex) = thumb {
        let s = tex.size_vec2();
        let k = ((well.width() - 40.0) / s.x).min((well.height() - 40.0) / s.y);
        let r = egui::Rect::from_center_size(well.center(), s * k);
        ui.painter().image(
            tex.id(),
            r,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    ui.add_space(10.0);
    let name = display_name(app, &t);
    ui.label(
        RichText::new(&name)
            .font(theme::bold(18.0))
            .color(theme::pal().text),
    );
    ui.label(
        RichText::new(format!("{} × {} mm", mm(t.width_mm), mm(t.height_mm)))
            .color(theme::pal().muted),
    );
    ui.add_space(6.0);
    ui.separator();
    let folder = if starter {
        app.i18n.t("templates-starters")
    } else if t.folder.is_empty() {
        app.i18n.t("templates-default-folder")
    } else {
        t.folder.clone()
    };
    let fact = |ui: &mut Ui, k: String, v: String| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(k).size(12.5).color(theme::pal().muted));
            theme::trailing(ui, |ui| ui.label(RichText::new(v).size(12.5)));
        });
    };
    fact(ui, app.i18n.t("templates-folder"), folder);
    if let Some(m) = &t.printer_model {
        let models = app.session.services().devices.models();
        let shown = models
            .iter()
            .find(|x| &x.id == m)
            .map_or_else(|| m.clone(), |x| x.name.clone());
        fact(ui, app.i18n.t("prop-printer-model"), shown);
    }
    ui.add_space(10.0);
    let full = ui.available_width();
    if icons::text_button(
        ui,
        "folder",
        &app.i18n.t("templates-open"),
        Some(theme::pal().primary),
        egui::Color32::WHITE,
        Some(full),
    )
    .clicked()
    {
        open_template(app, &sel);
    }
    ui.add_space(6.0);
    let link = |ui: &mut Ui, icon: &str, text: String, color: egui::Color32| {
        icons::text_button(
            ui,
            icon,
            &text,
            Some(theme::pal().surface),
            color,
            Some(full),
        )
    };
    if link(
        ui,
        "copy",
        app.i18n.t("templates-duplicate"),
        theme::pal().text,
    )
    .clicked()
    {
        let folder = match &g.shelf {
            Shelf::Folder(f) => f.clone(),
            Shelf::Starters => String::new(),
        };
        let env = app.run_reporting("template.duplicate", json!({"ref": sel, "folder": folder}));
        if let Some(r) = env.result.as_ref().and_then(|r| r["ref"].as_str()) {
            g.selected = Some(r.to_owned());
            if starter {
                g.shelf = Shelf::Folder(String::new());
            }
            g.stale = true;
        }
    }
    if starter {
        return;
    }
    ui.horizontal(|ui| {
        let w = ui.available_width() - 90.0;
        ui.add(
            egui::TextEdit::singleline(&mut g.rename)
                .hint_text(app.i18n.t("prop-name"))
                .desired_width(w),
        );
        if ui.button(app.i18n.t("templates-rename")).clicked() {
            let env = app.run_reporting(
                "template.move",
                json!({"ref": sel, "name": g.rename.trim()}),
            );
            if let Some(r) = env.result.as_ref().and_then(|r| r["ref"].as_str()) {
                g.selected = Some(r.to_owned());
                g.stale = true;
            }
        }
    });
    let mut target: Option<String> = None;
    ui.menu_button(
        RichText::new(format!("🗀  {}", app.i18n.t("templates-move-to"))),
        |ui| {
            if ui.button(app.i18n.t("templates-default-folder")).clicked() {
                target = Some(String::new());
            }
            for f in &g.folders {
                if ui.button(f).clicked() {
                    target = Some(f.clone());
                }
            }
        },
    );
    if let Some(folder) = target {
        let env = app.run_reporting("template.move", json!({"ref": sel, "folder": folder}));
        if let Some(r) = env.result.as_ref().and_then(|r| r["ref"].as_str()) {
            g.selected = Some(r.to_owned());
            g.shelf = Shelf::Folder(folder);
            g.stale = true;
        }
    }
    if link(
        ui,
        "trash",
        app.i18n.t("templates-delete"),
        theme::pal().red,
    )
    .clicked()
        && app.run_reporting("template.delete", json!({"ref": sel})).ok
    {
        g.selected = None;
        g.stale = true;
    }
}

/// "Save as template"; returns true when it should close.
pub fn save(app: &mut LabelyApp, ui: &mut Ui, f: &mut SaveForm) -> bool {
    ui.heading(app.i18n.t("cmd-template-save"));
    egui::Grid::new("save-template")
        .num_columns(2)
        .show(ui, |ui| {
            let l = ui.label(app.i18n.t("prop-name")).id;
            if ui
                .text_edit_singleline(&mut f.name)
                .labelled_by(l)
                .changed()
            {
                f.exists = false;
            }
            ui.end_row();
            let l = ui.label(app.i18n.t("templates-folder")).id;
            ui.vertical(|ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut f.folder)
                            .hint_text(app.i18n.t("templates-mine")),
                    )
                    .labelled_by(l)
                    .changed()
                {
                    f.exists = false;
                }
                ui.horizontal_wrapped(|ui| {
                    for name in f.folders.clone() {
                        if ui.small_button(&name).clicked() {
                            f.folder = name;
                            f.exists = false;
                        }
                    }
                });
            });
            ui.end_row();
        });
    if f.exists {
        ui.colored_label(ui.visuals().warn_fg_color, app.i18n.t("templates-exists"));
    }
    ui.separator();
    let mut close = false;
    ui.horizontal(|ui| {
        let label = if f.exists {
            "templates-replace"
        } else {
            "action-save"
        };
        if ui.button(app.i18n.t(label)).clicked() {
            let p = json!({
                "name": f.name.trim(),
                "folder": f.folder.trim(),
                "overwrite": f.exists,
            });
            let env = app.run("template.save", p);
            match &env.error {
                None => {
                    app.set_status(app.i18n.t("status-template-saved"), false);
                    close = true;
                }
                Some(e) if e.message.contains("already exists") => f.exists = true,
                Some(e) => {
                    let text = format!("{}: {}", app.i18n.error_title(e.code.as_str()), e.message);
                    app.set_status(text, true);
                }
            }
        }
        if ui.button(app.i18n.t("action-cancel")).clicked() {
            close = true;
        }
    });
    close
}
