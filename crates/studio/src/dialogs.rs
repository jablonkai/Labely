//! Modal dialogs: new document, templates, print, about.

use egui::{Context, Modal, Ui};
use labely_i18n::Arg;
use serde_json::json;

use crate::app::{Dialog, LabelyApp};

/// `doc.new` form.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDocForm {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub printer_model: String,
}

impl Default for NewDocForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            width_mm: 50.0,
            height_mm: 30.0,
            printer_model: "niimbot.b21_pro".to_owned(),
        }
    }
}

/// Print dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintForm {
    pub address: String,
    pub copies: u16,
    /// 0 = model default.
    pub density: u8,
    /// Data records, e.g. `1,3,5-7`; empty = all.
    pub records: String,
    /// Labels to print without data.
    pub count: u32,
    /// Print offset in mm, right and down.
    pub offset_x_mm: f32,
    pub offset_y_mm: f32,
    /// Mirror the print left to right.
    pub mirror: bool,
    /// Turn the print 180°.
    pub rotate_180: bool,
    /// Data column holding the copies of each record; empty = `copies` for every record.
    pub copies_column: String,
}

impl Default for PrintForm {
    fn default() -> Self {
        let dir = std::env::temp_dir().join("labely-fake-printer");
        Self {
            address: format!("fake:{}", dir.display()),
            copies: 1,
            density: 0,
            records: String::new(),
            count: 1,
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            mirror: false,
            rotate_180: false,
            copies_column: String::new(),
        }
    }
}

/// Show the open dialog, if any.
pub fn show(app: &mut LabelyApp, ctx: &Context) {
    let Some(dialog) = app.ui.dialog.clone() else {
        return;
    };
    let mut close = false;
    let mut next = dialog.clone();
    // One id per dialog kind, so each sizes itself instead of inheriting another's size.
    let kind = std::mem::discriminant(&dialog);
    Modal::new(egui::Id::unique(("labely-dialog", kind))).show(ctx, |ui| {
        ui.set_min_width(320.0);
        close = match &mut next {
            Dialog::NewDoc(form) => new_doc(app, ui, form),
            Dialog::Templates(g) => crate::templates::gallery(app, ui, g),
            Dialog::SaveTemplate(f) => crate::templates::save(app, ui, f),
            Dialog::History(v) => crate::jobs::history(app, ui, v),
            Dialog::Papers(v) => crate::paper::library(app, ui, v),
            Dialog::Device(v) => crate::paper::device(app, ui, v),
            Dialog::Print => print(app, ui),
            Dialog::About => about(app, ui),
            Dialog::ConfirmClose(doc_id) => confirm_close(app, ui, doc_id),
        };
    });
    app.ui.dialog = if close { None } else { Some(next) };
}

/// Returns true when the dialog should close.
fn new_doc(app: &mut LabelyApp, ui: &mut Ui, f: &mut NewDocForm) -> bool {
    ui.heading(app.i18n.t("cmd-doc-new"));
    egui::Grid::new("newdoc").num_columns(2).show(ui, |ui| {
        let l = ui.label(app.i18n.t("prop-name")).id;
        ui.text_edit_singleline(&mut f.name).labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-width-mm")).id;
        ui.add(crate::panels::length_drag(&mut f.width_mm, app.units()).range(5.0..=300.0))
            .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-height-mm")).id;
        ui.add(crate::panels::length_drag(&mut f.height_mm, app.units()).range(5.0..=1000.0))
            .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-printer-model")).id;
        let models = app.session.services().devices.models();
        let shown = models
            .iter()
            .find(|m| m.id == f.printer_model)
            .map_or_else(|| app.i18n.t("model-none"), |m| m.name.clone());
        egui::ComboBox::from_id_salt("new_model")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut f.printer_model,
                    String::new(),
                    app.i18n.t("model-none"),
                );
                for m in &models {
                    ui.selectable_value(&mut f.printer_model, m.id.clone(), &m.name);
                }
            })
            .response
            .labelled_by(l);
        ui.end_row();
    });
    ui.separator();
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("action-create")).clicked() {
            let mut p = json!({"width_mm": f.width_mm, "height_mm": f.height_mm});
            if !f.name.trim().is_empty() {
                p["name"] = json!(f.name.trim());
            }
            if !f.printer_model.is_empty() {
                p["printer_model"] = json!(f.printer_model);
            }
            if app.run_reporting("doc.new", p).ok {
                app.ui.fit_pending = true;
                close = true;
            }
        }
        if ui.button(app.i18n.t("action-cancel")).clicked() {
            close = true;
        }
    });
    close
}

fn print(app: &mut LabelyApp, ui: &mut Ui) -> bool {
    ui.heading(app.i18n.t("cmd-print-submit"));
    let connected = app.session.services().devices.connected();
    let mut form = app.ui.print.clone();
    egui::Grid::new("print").num_columns(2).show(ui, |ui| {
        let l = ui.label(app.i18n.t("print-printer")).id;
        ui.vertical(|ui| {
            ui.text_edit_singleline(&mut form.address).labelled_by(l);
            for d in &connected {
                if ui.small_button(format!("🖶 {}", d.name)).clicked() {
                    form.address = d.address.clone();
                }
            }
        });
        ui.end_row();
        let l = ui.label(app.i18n.t("print-copies")).id;
        ui.add(egui::DragValue::new(&mut form.copies).range(1..=999))
            .labelled_by(l);
        ui.end_row();
        let columns: Option<Vec<String>> = app
            .session
            .doc(None)
            .and_then(|d| d.data.as_ref().map(|s| s.columns.clone()));
        if let Some(columns) = &columns {
            if !form.copies_column.is_empty() && !columns.contains(&form.copies_column) {
                form.copies_column.clear();
            }
            let l = ui.label(app.i18n.t("print-copies-column")).id;
            let none = app.i18n.t("print-copies-column-none");
            let shown = if form.copies_column.is_empty() {
                none.clone()
            } else {
                form.copies_column.clone()
            };
            egui::ComboBox::from_id_salt("print-copies-column")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.copies_column, String::new(), none);
                    for c in columns {
                        ui.selectable_value(&mut form.copies_column, c.clone(), c);
                    }
                })
                .response
                .labelled_by(l)
                .on_hover_text(app.i18n.t("print-copies-column-hint"));
            ui.end_row();
            let l = ui.label(app.i18n.t("print-records")).id;
            ui.add(
                egui::TextEdit::singleline(&mut form.records)
                    .hint_text(app.i18n.t("print-records-hint")),
            )
            .labelled_by(l);
        } else {
            let l = ui.label(app.i18n.t("print-count")).id;
            ui.add(egui::DragValue::new(&mut form.count).range(1..=5000))
                .labelled_by(l);
        }
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-density")).id;
        ui.add(
            egui::Slider::new(&mut form.density, 0..=5).text(app.i18n.t("print-density-default")),
        )
        .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("print-offset")).id;
        ui.horizontal(|ui| {
            for (v, key) in [
                (&mut form.offset_x_mm, "print-offset-x"),
                (&mut form.offset_y_mm, "print-offset-y"),
            ] {
                let mut mm = f64::from(*v);
                let r = ui.add(
                    crate::panels::length_drag(&mut mm, app.units())
                        .range(-10.0..=10.0)
                        .prefix(format!("{} ", app.i18n.t(key))),
                );
                *v = mm as f32;
                r.labelled_by(l);
            }
        });
        ui.end_row();
        ui.label("");
        ui.vertical(|ui| {
            ui.checkbox(&mut form.mirror, app.i18n.t("print-mirror"));
            ui.checkbox(&mut form.rotate_180, app.i18n.t("print-rotate-180"));
        });
        ui.end_row();
    });
    print_preview(app, ui, &form);
    app.ui.print = form.clone();
    ui.separator();
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("action-print")).clicked() {
            let connected = connected.iter().any(|d| d.address == form.address);
            let ok = connected || crate::menu::connect(app, &form.address);
            if ok {
                let mut p = json!({"address": form.address, "copies": form.copies, "wait": false});
                if form.density > 0 {
                    p["density"] = json!(form.density);
                }
                if app.session.doc(None).is_some_and(|d| d.data.is_some()) {
                    if !form.records.trim().is_empty() {
                        p["records"] = json!(form.records.trim());
                    }
                    if !form.copies_column.is_empty() {
                        p["copies_column"] = json!(form.copies_column);
                    }
                } else if form.count > 1 {
                    p["count"] = json!(form.count);
                }
                if form.offset_x_mm != 0.0 {
                    p["offset_x_mm"] = json!(form.offset_x_mm);
                }
                if form.offset_y_mm != 0.0 {
                    p["offset_y_mm"] = json!(form.offset_y_mm);
                }
                if form.mirror {
                    p["mirror"] = json!(true);
                }
                if form.rotate_180 {
                    p["rotate_180"] = json!(true);
                }
                if app.run_reporting("print.submit", p).ok {
                    app.set_status(app.i18n.t("status-queued"), false);
                    app.ui.show_printers = true;
                    close = true;
                }
            }
        }
        if ui.button(app.i18n.t("action-cancel")).clicked() {
            close = true;
        }
    });
    close
}

/// Save, discard or keep a document with unsaved changes.
fn confirm_close(app: &mut LabelyApp, ui: &mut Ui, doc_id: &str) -> bool {
    let name = app
        .session
        .doc(Some(doc_id))
        .map_or_else(String::new, |d| d.name.clone());
    ui.heading(app.i18n.t("close-title"));
    ui.label(
        app.i18n
            .t_args("close-unsaved", &[("name", Arg::Str(&name))]),
    );
    ui.add_space(8.0);
    let mut done = false;
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("action-save")).clicked() {
            app.activate(doc_id);
            crate::menu::save(app);
            if !app.session.is_dirty(Some(doc_id)) {
                app.close_doc(doc_id);
            }
            done = true;
        }
        if ui.button(app.i18n.t("action-discard")).clicked() {
            app.close_doc(doc_id);
            done = true;
        }
        if ui.button(app.i18n.t("action-cancel")).clicked() {
            done = true;
        }
    });
    done
}

fn about(app: &mut LabelyApp, ui: &mut Ui) -> bool {
    ui.heading("Labely");
    ui.label(format!(
        "{} {}",
        app.i18n.t("about-version"),
        env!("CARGO_PKG_VERSION")
    ));
    ui.label(app.i18n.t("about-text"));
    if let Some(h) = &app.mcp {
        ui.label(format!("MCP: {}", h.url));
        ui.label(format!("{}", h.conn_path.display()));
    }
    ui.button(app.i18n.t("action-close")).clicked()
}

/// Longest side of the preview in the print dialog, points.
const PREVIEW_MAX: f32 = 260.0;

/// The label as it leaves the printer: the print raster with the dialog's mirror and 180°
/// turn applied (offsets are too small to show at this size).
fn print_preview(app: &LabelyApp, ui: &mut Ui, form: &PrintForm) {
    let Some(r) = &app.raster else {
        return;
    };
    let (w, h) = (r.size[0] as f32, r.size[1] as f32);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = (PREVIEW_MAX / w.max(h)).min(1.0);
    // Mirroring flips x; a 180° turn flips both axes.
    let flip_x = form.mirror != form.rotate_180;
    let flip_y = form.rotate_180;
    let (u0, u1) = if flip_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let (v0, v1) = if flip_y { (1.0, 0.0) } else { (0.0, 1.0) };
    ui.separator();
    ui.label(egui::RichText::new(app.i18n.t("print-preview")).weak());
    ui.add(
        egui::Image::new((r.texture.id(), egui::vec2(w * scale, h * scale)))
            .uv(egui::Rect::from_min_max(
                egui::pos2(u0, v0),
                egui::pos2(u1, v1),
            ))
            .bg_fill(egui::Color32::WHITE),
    );
}
