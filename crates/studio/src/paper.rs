//! Label paper library dialog, loaded-paper banner and printer settings dialog (ADR-0020).
//! Everything goes through `paper.*` and `device.*` commands.

use egui::{RichText, Ui};
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::{Dialog, LabelyApp};

/// Media types offered in the forms, with their i18n keys.
pub const MEDIA: [(&str, &str); 4] = [
    ("gap", "media-gap"),
    ("black_mark", "media-black-mark"),
    ("continuous", "media-continuous"),
    ("transparent", "media-transparent"),
];

fn media_label(app: &LabelyApp, media: &str) -> String {
    MEDIA
        .iter()
        .find(|(m, _)| *m == media)
        .map_or_else(|| media.to_owned(), |(_, k)| app.i18n.t(k))
}

fn media_combo(
    app: &LabelyApp,
    ui: &mut Ui,
    label: egui::Id,
    id: &str,
    value: &mut String,
    with_roll: bool,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(media_label(app, value))
        .show_ui(ui, |ui| {
            for (m, key) in MEDIA {
                if with_roll || m != "continuous" {
                    ui.selectable_value(value, m.to_owned(), app.i18n.t(key));
                }
            }
        })
        .response
        .labelled_by(label);
}

/// `12.5` → `12.5`, `50.0` → `50`.
fn mm(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
}

/// "50 × 30 mm" for a paper JSON object.
pub fn size_text(p: &Value) -> String {
    format!(
        "{} × {} mm",
        mm(p["width_mm"].as_f64().unwrap_or(0.0)),
        mm(p["height_mm"].as_f64().unwrap_or(0.0))
    )
}

/// Paper library dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperView {
    pub query: String,
    /// Only papers that fit the label's printer model.
    pub fitting: bool,
    pub papers: Vec<Value>,
    /// RFID barcode to link to a paper (from the loaded-paper banner).
    pub link_barcode: Option<String>,
    pub new_name: String,
    pub new_width_mm: f64,
    pub new_height_mm: f64,
    pub new_radius_mm: f64,
    pub new_media: String,
}

impl Default for PaperView {
    fn default() -> Self {
        Self {
            query: String::new(),
            fitting: true,
            papers: Vec::new(),
            link_barcode: None,
            new_name: String::new(),
            new_width_mm: 40.0,
            new_height_mm: 30.0,
            new_radius_mm: 0.0,
            new_media: "gap".to_owned(),
        }
    }
}

impl PaperView {
    fn reload(&mut self, app: &mut LabelyApp) {
        let mut p = json!({ "query": self.query });
        if self.fitting
            && let Some(model) = app
                .session
                .doc(None)
                .and_then(|d| d.label.printer_model.clone())
        {
            p["model"] = json!(model);
        }
        let env = app.run_reporting("paper.list", p);
        self.papers = env
            .result
            .and_then(|r| r["papers"].as_array().cloned())
            .unwrap_or_default();
    }
}

/// Open the library; with `link_barcode`, rows offer to link that RFID barcode.
pub fn open_library(app: &mut LabelyApp, link_barcode: Option<String>) {
    let mut view = PaperView {
        link_barcode,
        ..PaperView::default()
    };
    view.reload(app);
    app.ui.dialog = Some(Dialog::Papers(view));
}

/// Returns true when the dialog should close.
pub fn library(app: &mut LabelyApp, ui: &mut Ui, v: &mut PaperView) -> bool {
    ui.heading(app.i18n.t("papers-title"));
    let mut close = false;
    let mut reload = false;
    if let Some(b) = &v.link_barcode {
        ui.label(
            RichText::new(
                app.i18n
                    .t_args("papers-link-hint", &[("barcode", Arg::Str(b))]),
            )
            .strong(),
        );
    }
    ui.horizontal(|ui| {
        let l = ui.label(app.i18n.t("papers-search")).id;
        if ui
            .text_edit_singleline(&mut v.query)
            .labelled_by(l)
            .changed()
        {
            reload = true;
        }
        if ui
            .checkbox(&mut v.fitting, app.i18n.t("papers-fitting"))
            .changed()
        {
            reload = true;
        }
    });
    let has_doc = app.session.active_doc_id().is_some();
    let label = app.session.doc(None).map(|d| d.label.clone());
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .id_salt("papers")
        .show(ui, |ui| {
            egui::Grid::new("papers-grid")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    for p in &v.papers {
                        let id = p["id"].as_str().unwrap_or("").to_owned();
                        let current = label.as_ref().is_some_and(|l| {
                            (l.width_mm - p["width_mm"].as_f64().unwrap_or(0.0)).abs() < 0.01
                                && (l.height_mm - p["height_mm"].as_f64().unwrap_or(0.0)).abs()
                                    < 0.01
                                && serde_json::to_value(l.media).ok().as_ref() == Some(&p["media"])
                        });
                        let mut name = RichText::new(p["name"].as_str().unwrap_or(""));
                        if current {
                            name = name.strong();
                        }
                        let r = ui.label(name);
                        if let Some(codes) = p["barcodes"].as_array() {
                            let codes: Vec<_> = codes.iter().filter_map(Value::as_str).collect();
                            r.on_hover_text(format!("RFID: {}", codes.join(", ")));
                        }
                        ui.label(size_text(p));
                        ui.label(media_label(app, p["media"].as_str().unwrap_or("")));
                        ui.horizontal(|ui| {
                            if has_doc
                                && ui.small_button(app.i18n.t("papers-use")).clicked()
                                && app.run_reporting("paper.apply", json!({"id": id})).ok
                            {
                                app.ui.fit_pending = true;
                                if v.link_barcode.is_none() {
                                    close = true;
                                }
                            }
                            if let Some(b) = v.link_barcode.clone()
                                && ui.small_button(app.i18n.t("papers-link")).clicked()
                                && app
                                    .run_reporting("paper.link", json!({"barcode": b, "id": id}))
                                    .ok
                            {
                                app.set_status(app.i18n.t("status-paper-linked"), false);
                                v.link_barcode = None;
                                app.ui.detected = None;
                                detect(app, None);
                                reload = true;
                            }
                            if p["custom"] == true
                                && ui.small_button(app.i18n.t("templates-delete")).clicked()
                                && app.run_reporting("paper.delete", json!({"id": id})).ok
                            {
                                reload = true;
                            }
                        });
                        ui.end_row();
                    }
                });
            if v.papers.is_empty() {
                ui.label(RichText::new(app.i18n.t("papers-none")).weak());
            }
        });
    ui.separator();
    ui.label(RichText::new(app.i18n.t("papers-add")).strong());
    egui::Grid::new("paper-add").num_columns(2).show(ui, |ui| {
        let l = ui.label(app.i18n.t("prop-name")).id;
        ui.text_edit_singleline(&mut v.new_name).labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-width-mm")).id;
        ui.add(crate::panels::length_drag(&mut v.new_width_mm, app.units()).range(5.0..=300.0))
            .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-height-mm")).id;
        ui.add(crate::panels::length_drag(&mut v.new_height_mm, app.units()).range(5.0..=1000.0))
            .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-corner-radius-mm")).id;
        ui.add(crate::panels::length_drag(&mut v.new_radius_mm, app.units()).range(0.0..=50.0))
            .labelled_by(l);
        ui.end_row();
        let l = ui.label(app.i18n.t("prop-media")).id;
        media_combo(app, ui, l, "paper-add-media", &mut v.new_media, true);
        ui.end_row();
    });
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("papers-add-button")).clicked() {
            let mut p = json!({
                "name": v.new_name,
                "width_mm": v.new_width_mm,
                "height_mm": v.new_height_mm,
                "corner_radius_mm": v.new_radius_mm,
                "media": v.new_media,
            });
            if let Some(b) = &v.link_barcode {
                p["barcode"] = json!(b);
            }
            if app.run_reporting("paper.add", p).ok {
                v.new_name.clear();
                if v.link_barcode.take().is_some() {
                    app.ui.detected = None;
                    detect(app, None);
                }
                reload = true;
            }
        }
        if ui.button(app.i18n.t("action-close")).clicked() {
            close = true;
        }
    });
    if reload {
        v.reload(app);
    }
    close
}

/// Read the loaded paper of `address` (default: the single connected printer) into
/// `app.ui.detected`. Quiet: a printer without RFID is not an error.
pub fn detect(app: &mut LabelyApp, address: Option<&str>) {
    let mut p = json!({});
    if let Some(a) = address {
        p["address"] = json!(a);
    }
    let env = app.run("paper.detect", p);
    app.ui.detected = env.result.filter(|r| r["rfid"] == true);
}

/// [`detect`], then apply a recognised paper that differs from the label when the user turned
/// "Use the detected paper automatically" on (ADR-0041). The edit is one undo step. Called
/// where a roll is read on purpose: on connect and from Print → Detect paper.
pub fn detect_and_apply(app: &mut LabelyApp, address: Option<&str>) {
    detect(app, address);
    if !app.prefs.auto_apply_paper {
        return;
    }
    let Some(d) = app.ui.detected.clone() else {
        return;
    };
    let (Some(id), false) = (d["paper"]["id"].as_str(), d["matches_label"] != false) else {
        return;
    };
    if app.run_reporting("paper.apply", json!({"id": id})).ok {
        app.ui.fit_pending = true;
        detect(app, d["address"].as_str());
        let name = d["paper"]["name"].as_str().unwrap_or(id);
        app.set_status(
            app.i18n
                .t_args("status-paper-applied", &[("name", Arg::Str(name))]),
            false,
        );
    }
}

/// Loaded-paper banner for the printers panel: offers to use a linked paper that differs
/// from the label, or to link an unknown barcode.
pub fn banner(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(d) = app.ui.detected.clone() else {
        return;
    };
    let barcode = d["barcode"].as_str().unwrap_or("").to_owned();
    let remaining = d["remaining"].as_f64().unwrap_or(0.0);
    ui.group(|ui| {
        if let Some(p) = d.get("paper") {
            ui.label(app.i18n.t_args(
                "paper-loaded",
                &[
                    ("name", Arg::Str(p["name"].as_str().unwrap_or(""))),
                    ("count", Arg::Num(remaining)),
                ],
            ));
            if d["matches_label"] == false {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(app.i18n.t("paper-mismatch")).weak());
                    let id = p["id"].as_str().unwrap_or("").to_owned();
                    if ui.small_button(app.i18n.t("papers-use")).clicked()
                        && app.run_reporting("paper.apply", json!({"id": id})).ok
                    {
                        app.ui.fit_pending = true;
                        detect(app, d["address"].as_str());
                    }
                });
            }
        } else {
            ui.label(app.i18n.t_args(
                "paper-unknown",
                &[
                    ("barcode", Arg::Str(&barcode)),
                    ("count", Arg::Num(remaining)),
                ],
            ));
            if ui.small_button(app.i18n.t("papers-link-button")).clicked() {
                open_library(app, Some(barcode));
            }
        }
    });
}

/// Printer settings dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceView {
    pub address: String,
    /// As read; `None` fields are not supported by the printer.
    pub auto_off_level: Option<u8>,
    pub sound_power: Option<bool>,
    pub sound_bluetooth: Option<bool>,
    /// Paper type to calibrate for.
    pub media: String,
}

/// Open the settings dialog for `address`, or the single connected printer.
pub fn open_device(app: &mut LabelyApp, address: Option<String>) {
    let Some(address) = address.or_else(|| {
        let c = app.session.services().devices.connected();
        (c.len() == 1).then(|| c[0].address.clone())
    }) else {
        app.ui.show_printers = true;
        app.set_status(app.i18n.t("status-pick-printer"), true);
        return;
    };
    let env = app.run_reporting("device.settings", json!({"address": address}));
    let s = env
        .result
        .map(|r| r["settings"].clone())
        .unwrap_or(Value::Null);
    let media = app
        .session
        .doc(None)
        .and_then(|d| serde_json::to_value(d.label.media).ok())
        .and_then(|m| m.as_str().map(str::to_owned))
        .filter(|m| m != "continuous")
        .unwrap_or_else(|| "gap".to_owned());
    app.ui.dialog = Some(Dialog::Device(DeviceView {
        address,
        auto_off_level: s["auto_off_level"].as_u64().map(|l| l as u8),
        sound_power: s["sound_power"].as_bool(),
        sound_bluetooth: s["sound_bluetooth"].as_bool(),
        media,
    }));
}

/// Returns true when the dialog should close.
pub fn device(app: &mut LabelyApp, ui: &mut Ui, v: &mut DeviceView) -> bool {
    ui.heading(app.i18n.t("device-title"));
    ui.label(RichText::new(&v.address).weak());
    let mut close = false;
    let before = (v.auto_off_level, v.sound_power, v.sound_bluetooth);
    egui::Grid::new("device-settings")
        .num_columns(2)
        .show(ui, |ui| {
            let label = ui.label(app.i18n.t("device-auto-off")).id;
            match v.auto_off_level.as_mut() {
                Some(level) => {
                    egui::ComboBox::from_id_salt("auto-off")
                        .selected_text(app.i18n.t(&format!("auto-off-{level}")))
                        .show_ui(ui, |ui| {
                            for l in 1..=4u8 {
                                ui.selectable_value(level, l, app.i18n.t(&format!("auto-off-{l}")));
                            }
                        })
                        .response
                        .labelled_by(label);
                }
                None => {
                    ui.label(RichText::new(app.i18n.t("device-unsupported")).weak());
                }
            }
            ui.end_row();
            for (key, value) in [
                ("device-sound-power", &mut v.sound_power),
                ("device-sound-bluetooth", &mut v.sound_bluetooth),
            ] {
                let label = ui.label(app.i18n.t(key)).id;
                match value.as_mut() {
                    Some(on) => {
                        ui.checkbox(on, "").labelled_by(label);
                    }
                    None => {
                        ui.label(RichText::new(app.i18n.t("device-unsupported")).weak());
                    }
                }
                ui.end_row();
            }
        });
    let after = (v.auto_off_level, v.sound_power, v.sound_bluetooth);
    if before != after {
        let mut p = json!({"address": v.address});
        if before.0 != after.0 {
            p["auto_off_level"] = json!(after.0);
        }
        if before.1 != after.1 {
            p["sound_power"] = json!(after.1);
        }
        if before.2 != after.2 {
            p["sound_bluetooth"] = json!(after.2);
        }
        let env = app.run_reporting("device.set_settings", p);
        match env.result {
            Some(r) => {
                let s = &r["settings"];
                v.auto_off_level = s["auto_off_level"].as_u64().map(|l| l as u8);
                v.sound_power = s["sound_power"].as_bool();
                v.sound_bluetooth = s["sound_bluetooth"].as_bool();
                app.set_status(app.i18n.t("status-settings-saved"), false);
            }
            None => (v.auto_off_level, v.sound_power, v.sound_bluetooth) = before,
        }
    }
    ui.separator();
    ui.label(RichText::new(app.i18n.t("device-calibrate")).strong());
    ui.label(RichText::new(app.i18n.t("device-calibrate-hint")).weak());
    ui.horizontal(|ui| {
        let l = ui.label(app.i18n.t("prop-media")).id;
        media_combo(app, ui, l, "calibrate-media", &mut v.media, false);
        if ui.button(app.i18n.t("device-calibrate-button")).clicked()
            && app
                .run_reporting(
                    "device.calibrate",
                    json!({"address": v.address, "media": v.media}),
                )
                .ok
        {
            app.set_status(app.i18n.t("status-calibrated"), false);
        }
    });
    ui.separator();
    if ui.button(app.i18n.t("action-close")).clicked() {
        close = true;
    }
    close
}
