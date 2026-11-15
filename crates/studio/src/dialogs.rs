//! Modal dialogs: new document, templates, print, about.

use egui::{Context, Modal, Ui};
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
            Dialog::Print => print(app, ui),
            Dialog::About => about(app, ui),
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
        ui.add(
            egui::DragValue::new(&mut f.width_mm)
                .range(5.0..=300.0)
                .speed(0.5),
        )
        .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-height-mm")).id;
        ui.add(
            egui::DragValue::new(&mut f.height_mm)
                .range(5.0..=1000.0)
                .speed(0.5),
        )
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
        let has_data = app.session.doc(None).is_some_and(|d| d.data.is_some());
        if has_data {
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
    });
    app.ui.print = form.clone();
    ui.separator();
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("action-print")).clicked() {
            let connected = connected.iter().any(|d| d.address == form.address);
            let ok = connected
                || app
                    .run_reporting("device.connect", json!({"address": form.address}))
                    .ok;
            if ok {
                let mut p = json!({"address": form.address, "copies": form.copies, "wait": false});
                if form.density > 0 {
                    p["density"] = json!(form.density);
                }
                if app.session.doc(None).is_some_and(|d| d.data.is_some()) {
                    if !form.records.trim().is_empty() {
                        p["records"] = json!(form.records.trim());
                    }
                } else if form.count > 1 {
                    p["count"] = json!(form.count);
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
