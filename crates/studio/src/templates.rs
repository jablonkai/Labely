//! Template gallery and "save as template" dialogs (ADR-0018). Every library change runs a
//! `template.*` command; the gallery only caches the listing and thumbnails.

use egui::Ui;
use labely_core::commands::template::{self, TemplateInfo};
use labely_core::services::RenderOptions;
use serde_json::json;

use crate::app::LabelyApp;

/// Which group the gallery shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shelf {
    Starters,
    /// A library folder; empty is the default folder.
    Folder(String),
}

/// Gallery state.
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

fn list(app: &mut LabelyApp) -> Option<(Vec<TemplateInfo>, Vec<String>)> {
    let env = app.run_reporting("template.list", json!({}));
    let r = env.result?;
    let templates = serde_json::from_value(r["templates"].clone()).ok()?;
    let folders = serde_json::from_value(r["folders"].clone()).unwrap_or_default();
    Some((templates, folders))
}

/// Grey thumbnail of a template, cached by reference.
fn thumbnail(
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

/// The gallery; returns true when it should close.
pub fn gallery(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) -> bool {
    if g.stale {
        if let Some((templates, folders)) = list(app) {
            g.templates = templates;
            g.folders = folders;
        }
        app.thumbs.clear();
        g.stale = false;
    }
    let mut close = false;
    ui.set_min_width(660.0);
    ui.set_max_width(660.0);
    ui.horizontal(|ui| {
        ui.heading(app.i18n.t("templates-title"));
        ui.add_space(16.0);
        ui.add(
            egui::TextEdit::singleline(&mut g.query)
                .hint_text(app.i18n.t("templates-search"))
                .desired_width(200.0),
        );
    });
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            shelves(app, ui, g);
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.set_width(450.0);
            close |= cards(app, ui, g);
        });
    });
    ui.separator();
    ui.horizontal(|ui| {
        close |= actions(app, ui, g);
        if ui.button(app.i18n.t("action-close")).clicked() {
            close = true;
        }
    });
    close
}

fn shelves(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) {
    let pick = |ui: &mut Ui, shelf: Shelf, text: String, g: &mut Gallery| {
        if ui.selectable_label(g.shelf == shelf, text).clicked() && g.shelf != shelf {
            if let Shelf::Folder(f) = &shelf {
                g.rename = f.clone();
            }
            g.shelf = shelf;
            g.selected = None;
        }
    };
    pick(ui, Shelf::Starters, app.i18n.t("templates-starters"), g);
    pick(
        ui,
        Shelf::Folder(String::new()),
        app.i18n.t("templates-mine"),
        g,
    );
    for f in g.folders.clone() {
        pick(ui, Shelf::Folder(f.clone()), format!("  🗀 {f}"), g);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut g.new_folder)
                .hint_text(app.i18n.t("templates-new-folder"))
                .desired_width(110.0),
        );
        if ui
            .add_enabled(!g.new_folder.trim().is_empty(), egui::Button::new("+"))
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
    {
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::singleline(&mut g.rename)
                .hint_text(app.i18n.t("templates-folder"))
                .desired_width(150.0),
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
            if ui.button(app.i18n.t("templates-delete-folder")).clicked()
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
            Shelf::Starters => t.starter,
            Shelf::Folder(f) => !t.starter && &t.folder == f,
        })
        .filter(|t| q.is_empty() || display_name(app, t).to_lowercase().contains(&q))
        .cloned()
        .collect()
}

fn cards(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) -> bool {
    let items = visible(app, g);
    if items.is_empty() {
        ui.label(app.i18n.t("templates-empty"));
        return false;
    }
    let ctx = ui.ctx().clone();
    let mut open = None;
    egui::ScrollArea::vertical()
        .max_height(380.0)
        .min_scrolled_height(380.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("template-cards").show(ui, |ui| {
                for (i, t) in items.iter().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        ui.end_row();
                    }
                    let selected = g.selected.as_deref() == Some(t.reference.as_str());
                    let thumb = thumbnail(app, &ctx, t);
                    let name = display_name(app, t);
                    let frame = egui::Frame::group(ui.style()).fill(if selected {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().extreme_bg_color
                    });
                    let resp = frame
                        .show(ui, |ui| {
                            ui.set_width(130.0);
                            ui.vertical_centered(|ui| {
                                let (area, _) = ui.allocate_exact_size(
                                    egui::vec2(120.0, 110.0),
                                    egui::Sense::hover(),
                                );
                                if let Some(tex) = &thumb {
                                    let s = tex.size_vec2();
                                    let k = (area.width() / s.x).min(area.height() / s.y).min(1.0);
                                    let rect = egui::Rect::from_center_size(area.center(), s * k);
                                    egui::Image::new((tex.id(), s * k)).paint_at(ui, rect);
                                }
                                ui.label(egui::RichText::new(&name).strong());
                                ui.small(format!("{} × {} mm", t.width_mm, t.height_mm));
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
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
    match open {
        Some(r) => open_template(app, &r),
        None => false,
    }
}

fn open_template(app: &mut LabelyApp, reference: &str) -> bool {
    let ok = app
        .run_reporting("template.new", json!({ "ref": reference }))
        .ok;
    if ok {
        app.ui.fit_pending = true;
        app.ui.record = 1;
    }
    ok
}

fn actions(app: &mut LabelyApp, ui: &mut Ui, g: &mut Gallery) -> bool {
    let Some(sel) = g.selected.clone() else {
        ui.label(app.i18n.t("templates-pick"));
        return false;
    };
    let starter = sel.starts_with("starter:");
    let mut close = false;
    if ui.button(app.i18n.t("templates-open")).clicked() {
        close = open_template(app, &sel);
    }
    if ui.button(app.i18n.t("templates-duplicate")).clicked() {
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
    if !starter {
        ui.add(
            egui::TextEdit::singleline(&mut g.rename)
                .hint_text(app.i18n.t("prop-name"))
                .desired_width(120.0),
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
        let mut target: Option<String> = None;
        ui.menu_button(app.i18n.t("templates-move-to"), |ui| {
            if ui.button(app.i18n.t("templates-mine")).clicked() {
                target = Some(String::new());
            }
            for f in &g.folders {
                if ui.button(f).clicked() {
                    target = Some(f.clone());
                }
            }
        });
        if let Some(folder) = target {
            let env = app.run_reporting("template.move", json!({"ref": sel, "folder": folder}));
            if let Some(r) = env.result.as_ref().and_then(|r| r["ref"].as_str()) {
                g.selected = Some(r.to_owned());
                g.shelf = Shelf::Folder(folder);
                g.stale = true;
            }
        }
        if ui.button(app.i18n.t("templates-delete")).clicked()
            && app.run_reporting("template.delete", json!({"ref": sel})).ok
        {
            g.selected = None;
            g.stale = true;
        }
    }
    close
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
