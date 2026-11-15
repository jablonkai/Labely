//! Menus and shortcuts, generated from `CommandMeta.menu` / `.shortcut`, and the actions that
//! turn a menu pick into a command (adding defaults, asking for a file, opening a dialog).

use egui::{Context, Key, KeyboardShortcut, Modifiers, Ui};
use labely_core::Registry;
use labely_i18n::{I18n, Lang};
use serde_json::{Value, json};

use crate::app::{Dialog, LabelyApp};
use crate::dialogs::NewDocForm;

/// Menu order.
pub const MENUS: &[&str] = &[
    "file", "edit", "insert", "arrange", "label", "data", "view", "print",
];

/// Localised label of a command: `cmd-<id>` with dots and underscores as dashes.
pub fn command_label(i18n: &I18n, id: &str) -> String {
    let key = format!("cmd-{}", id.replace(['.', '_'], "-"));
    if i18n.has(&key) {
        i18n.t(&key)
    } else {
        Registry::builtin()
            .get(id)
            .map_or_else(|| id.to_owned(), |e| e.meta.label.to_owned())
    }
}

/// Parse `Ctrl+Shift+Z`, `Delete`, `T` into an egui shortcut (`Ctrl` is Cmd on macOS).
pub fn parse_shortcut(s: &str) -> Option<KeyboardShortcut> {
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+') {
        match part {
            "Ctrl" | "Cmd" => mods |= Modifiers::COMMAND,
            "Shift" => mods |= Modifiers::SHIFT,
            "Alt" => mods |= Modifiers::ALT,
            k => key = Key::from_name(k),
        }
    }
    key.map(|k| KeyboardShortcut::new(mods, k))
}

/// Commands placed in `menu`, in registry order.
fn menu_commands(menu: &str) -> impl Iterator<Item = &'static labely_core::registry::Entry> + '_ {
    Registry::builtin()
        .entries()
        .iter()
        .filter(move |e| e.meta.menu == Some(menu))
}

/// Whether a menu item can run now.
fn enabled(app: &LabelyApp, id: &str) -> bool {
    let has_doc = app.session.active_doc_id().is_some();
    let has_sel = !app.selection().is_empty();
    match id {
        "doc.new" | "doc.open" | "template.new" | "device.scan" | "paper.list" => true,
        "paper.detect" | "device.calibrate" => {
            !app.session.services().devices.connected().is_empty()
        }
        "history.undo" => app.session.can_undo(None),
        "history.redo" => app.session.can_redo(None),
        "element.delete" | "element.duplicate" | "element.reorder" | "element.ungroup"
        | "element.lock" => has_sel,
        "element.group" => app.selection().len() >= 2,
        _ => has_doc,
    }
}

/// The top menu bar.
pub fn menu_bar(app: &mut LabelyApp, ui: &mut Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        for menu in MENUS {
            let title = app.i18n.t(&format!("menu-{menu}"));
            ui.menu_button(title, |ui| {
                for e in menu_commands(menu) {
                    let mut button = egui::Button::new(command_label(&app.i18n, e.id));
                    if let Some(sc) = e.meta.shortcut.and_then(parse_shortcut) {
                        button = button.shortcut_text(ui.ctx().format_shortcut(&sc));
                    }
                    if ui.add_enabled(enabled(app, e.id), button).clicked() {
                        invoke(app, e.id);
                        ui.close();
                    }
                }
                extra_items(app, ui, menu);
            });
        }
    });
}

/// Studio-only items (view state, settings) after the generated ones.
fn extra_items(app: &mut LabelyApp, ui: &mut Ui, menu: &str) {
    match menu {
        "insert" => {
            ui.separator();
            let has_doc = app.session.active_doc_id().is_some();
            for (key, text) in [
                ("action-insert-date", "{{now:%Y-%m-%d}}"),
                ("action-insert-time", "{{now:%H:%M}}"),
                ("action-insert-serial", "{{serial}}"),
            ] {
                if ui
                    .add_enabled(has_doc, egui::Button::new(app.i18n.t(key)))
                    .clicked()
                {
                    insert_bound_text(app, text);
                    ui.close();
                }
            }
            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(app.i18n.t("action-insert-code-from-image")),
                )
                .clicked()
            {
                ui.close();
                if let Some(path) = pick_file(app, &["png", "jpg", "jpeg", "bmp", "webp"]) {
                    insert_code_from_image(app, &json!({ "path": path }));
                }
            }
        }
        "edit" => {
            ui.separator();
            if ui
                .add_enabled(
                    app.session.active_doc_id().is_some(),
                    egui::Button::new(app.i18n.t("action-select-all")),
                )
                .clicked()
            {
                select_all(app);
                ui.close();
            }
        }
        "arrange" => {
            ui.separator();
            let n = app.selection().len();
            ui.add_enabled_ui(n >= 1, |ui| {
                ui.menu_button(app.i18n.t("menu-align"), |ui| {
                    for edge in ["left", "center", "right", "top", "middle", "bottom"] {
                        if ui.button(app.i18n.t(&format!("align-{edge}"))).clicked() {
                            let ids = app.selection();
                            app.run_reporting("element.align", json!({"ids": ids, "edge": edge}));
                            ui.close();
                        }
                    }
                });
            });
            for axis in ["horizontal", "vertical"] {
                if ui
                    .add_enabled(
                        n >= 3,
                        egui::Button::new(app.i18n.t(&format!("distribute-{axis}"))),
                    )
                    .clicked()
                {
                    let ids = app.selection();
                    app.run_reporting("element.distribute", json!({"ids": ids, "axis": axis}));
                    ui.close();
                }
            }
            ui.separator();
            ui.checkbox(&mut app.ui.snap, app.i18n.t("action-snap"));
        }
        "view" => {
            ui.separator();
            ui.checkbox(&mut app.ui.show_grid, app.i18n.t("action-show-grid"));
            ui.horizontal(|ui| {
                ui.label(app.i18n.t("prop-grid-mm"));
                ui.add(
                    egui::DragValue::new(&mut app.ui.grid_mm)
                        .range(0.5..=20.0)
                        .speed(0.1),
                );
            });
            ui.menu_button(app.i18n.t("menu-label-color"), |ui| {
                for (key, rgb) in [
                    ("color-white", [255, 255, 255]),
                    ("color-yellow", [255, 236, 140]),
                    ("color-pink", [255, 196, 214]),
                    ("color-blue", [180, 220, 255]),
                    ("color-green", [196, 236, 190]),
                    ("color-transparent", [228, 228, 228]),
                ] {
                    if ui
                        .radio(app.ui.label_color == rgb, app.i18n.t(key))
                        .clicked()
                    {
                        app.ui.label_color = rgb;
                        ui.close();
                    }
                }
            });
            if ui.button(app.i18n.t("action-zoom-in")).clicked() {
                app.ui.zoom = (app.ui.zoom * 1.25).min(80.0);
            }
            if ui.button(app.i18n.t("action-zoom-out")).clicked() {
                app.ui.zoom = (app.ui.zoom / 1.25).max(1.0);
            }
            if ui.button(app.i18n.t("action-zoom-fit")).clicked() {
                app.ui.fit_pending = true;
                ui.close();
            }
            ui.separator();
            ui.checkbox(&mut app.ui.show_layers, app.i18n.t("panel-layers"));
            ui.checkbox(&mut app.ui.show_props, app.i18n.t("panel-properties"));
            ui.checkbox(&mut app.ui.show_check, app.i18n.t("panel-check"));
            ui.checkbox(&mut app.ui.show_printers, app.i18n.t("panel-printers"));
            ui.checkbox(&mut app.ui.show_data, app.i18n.t("panel-data"));
            ui.checkbox(&mut app.ui.show_preview, app.i18n.t("panel-preview"));
            ui.separator();
            ui.menu_button(app.i18n.t("menu-language"), |ui| {
                for lang in Lang::ALL {
                    if ui
                        .radio(app.i18n.lang() == lang, lang.native_name())
                        .clicked()
                    {
                        app.i18n = I18n::new(lang);
                        ui.close();
                    }
                }
            });
            if ui.button(app.i18n.t("action-about")).clicked() {
                app.ui.dialog = Some(Dialog::About);
                ui.close();
            }
        }
        "print" => {
            ui.separator();
            ui.checkbox(
                &mut app.ui.allow_agent_print,
                app.i18n.t("action-allow-agent-print"),
            );
        }
        _ => {}
    }
}

/// Run shortcuts unless a text field has focus.
pub fn shortcuts(app: &mut LabelyApp, ctx: &Context) {
    if ctx.egui_wants_keyboard_input() || app.ui.dialog.is_some() {
        return;
    }
    let mut fired = Vec::new();
    for e in Registry::builtin().entries() {
        let Some(sc) = e.meta.shortcut.and_then(parse_shortcut) else {
            continue;
        };
        if enabled(app, e.id) && ctx.input_mut(|i| i.consume_shortcut(&sc)) {
            fired.push(e.id);
        }
    }
    for id in fired {
        invoke(app, id);
    }
    if app.session.active_doc_id().is_none() {
        return;
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::A))) {
        select_all(app);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        app.select(&[]);
    }
    let step = if ctx.input(|i| i.modifiers.shift) {
        5.0
    } else {
        0.5
    };
    let mut d = (0.0, 0.0);
    ctx.input(|i| {
        for (k, dx, dy) in [
            (Key::ArrowLeft, -1.0, 0.0),
            (Key::ArrowRight, 1.0, 0.0),
            (Key::ArrowUp, 0.0, -1.0),
            (Key::ArrowDown, 0.0, 1.0),
        ] {
            if i.key_pressed(k) {
                d = (d.0 + dx * step, d.1 + dy * step);
            }
        }
    });
    if d != (0.0, 0.0) {
        nudge(app, d.0, d.1);
    }
}

fn select_all(app: &mut LabelyApp) {
    let ids: Vec<_> = app
        .session
        .doc(None)
        .map(|d| d.elements().map(|e| e.id).collect())
        .unwrap_or_default();
    app.select(&ids);
}

fn nudge(app: &mut LabelyApp, dx: f64, dy: f64) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    for id in app.selection() {
        if let Some(e) = doc.element(id).filter(|e| !e.locked) {
            app.run(
                "element.move",
                json!({"id": id, "x_mm": e.x_mm + dx, "y_mm": e.y_mm + dy, "coalesce": "nudge"}),
            );
        }
    }
}

/// Default params for `element.add_*` at the middle of the label.
pub fn add_defaults(app: &LabelyApp, id: &str) -> Option<Value> {
    let doc = app.session.doc(None)?;
    let (w, h) = (doc.label.width_mm, doc.label.height_mm);
    let boxed = |bw: f64, bh: f64| {
        let (bw, bh) = (bw.min(w * 0.9), bh.min(h * 0.9));
        json!({"x_mm": ((w - bw) / 2.0 * 10.0).round() / 10.0, "y_mm": ((h - bh) / 2.0 * 10.0).round() / 10.0, "width_mm": bw, "height_mm": bh})
    };
    let mut p = match id {
        "element.add_text" => boxed(w * 0.8, 8.0),
        "element.add_barcode" => boxed(w * 0.7, h * 0.4),
        "element.add_code2d" => {
            let s = (h * 0.6).min(w * 0.6);
            boxed(s, s)
        }
        "element.add_rect" | "element.add_ellipse" => boxed(w * 0.4, h * 0.4),
        "element.add_line" => boxed(w * 0.6, 0.0),
        "element.add_image" => boxed(w * 0.4, h * 0.6),
        "element.add_table" => boxed(w * 0.8, h * 0.6),
        "element.add_icon" => {
            let s = (h * 0.5).min(w * 0.5);
            boxed(s, s)
        }
        "element.add_border" => json!({"x_mm": 0.0, "y_mm": 0.0, "width_mm": w, "height_mm": h}),
        _ => return None,
    };
    let extra = match id {
        "element.add_text" => json!({"text": app.i18n.t("default-text")}),
        "element.add_barcode" => json!({"data": "123456789012"}),
        "element.add_code2d" => json!({"data": "https://labely.org"}),
        "element.add_icon" => json!({"icon": "check"}),
        _ => json!({}),
    };
    if let (Some(p), Some(e)) = (p.as_object_mut(), extra.as_object()) {
        p.extend(e.clone());
    }
    Some(p)
}

/// Do what the menu item or shortcut for command `id` means.
pub fn invoke(app: &mut LabelyApp, id: &str) {
    match id {
        "doc.new" => {
            app.ui.dialog = Some(Dialog::NewDoc(NewDocForm::default()));
        }
        "doc.open" => {
            if let Some(path) = pick_file(app, &["labely"])
                && app.run_reporting("doc.open", json!({ "path": path })).ok
            {
                app.ui.fit_pending = true;
            }
        }
        "doc.save" => save(app),
        "template.new" => {
            app.ui.dialog = Some(Dialog::Templates(crate::templates::Gallery::default()));
        }
        "template.save" => {
            let form = crate::templates::SaveForm::new(app);
            app.ui.dialog = Some(Dialog::SaveTemplate(form));
        }
        "export.png" | "export.pdf" => export(app, id),
        "data.attach_images" => {
            if !app.opts.native_dialogs {
                app.set_status(app.i18n.t("status-no-dialogs"), true);
                return;
            }
            let Some(paths) = rfd::FileDialog::new()
                .add_filter("", &["png", "jpg", "jpeg", "bmp", "webp"])
                .pick_files()
            else {
                return;
            };
            if app.run_reporting(id, json!({ "paths": paths })).ok {
                app.ui.record = 1;
                app.ui.show_data = true;
            }
        }
        "doc.set_label" => app.select(&[]),
        "doc.check" => {
            let env = app.run_reporting("doc.check", json!({}));
            app.ui.check = env.result;
            app.ui.show_check = true;
        }
        "render.preview" => app.ui.show_preview = !app.ui.show_preview,
        "print.submit" => app.ui.dialog = Some(Dialog::Print),
        "job.history" => crate::jobs::open_history(app),
        "paper.list" => crate::paper::open_library(app, None),
        "paper.detect" => {
            app.ui.show_printers = true;
            crate::paper::detect(app, None);
            if app.ui.detected.is_none() {
                app.set_status(app.i18n.t("status-no-rfid"), false);
            }
        }
        "device.calibrate" => crate::paper::open_device(app, None),
        "device.scan" => {
            app.ui.show_printers = true;
            scan(app);
        }
        "selection.set" => select_all(app),
        "element.delete" => {
            let ids = app.selection();
            app.run_reporting("element.delete", json!({ "ids": ids }));
        }
        "element.duplicate" => {
            let mut created = Vec::new();
            for sel in app.selection() {
                let env = app.run_reporting(
                    "element.duplicate",
                    json!({"id": sel, "dx_mm": 1.0, "dy_mm": 1.0}),
                );
                created.extend(env.created);
            }
            app.select(&created);
        }
        "element.group" => {
            let ids = app.selection();
            app.run_reporting("element.group", json!({ "ids": ids }));
        }
        "element.ungroup" => {
            let ids = app.selection();
            app.run_reporting("element.ungroup", json!({ "ids": ids }));
        }
        "element.lock" => {
            let ids = app.selection();
            let all_locked = app.session.doc(None).is_some_and(|d| {
                ids.iter()
                    .all(|id| d.element(*id).is_some_and(|e| e.locked))
            });
            app.run_reporting("element.lock", json!({"ids": ids, "locked": !all_locked}));
        }
        "element.reorder" => {
            for sel in app.selection() {
                app.run_reporting("element.reorder", json!({"id": sel, "to": "front"}));
            }
        }
        "data.attach_csv" | "data.attach_xlsx" => {
            let exts: &[&str] = if id == "data.attach_csv" {
                &["csv", "tsv", "txt"]
            } else {
                &["xlsx", "xlsm", "xls", "ods"]
            };
            if let Some(path) = pick_file(app, exts)
                && app.run_reporting(id, json!({ "path": path })).ok
            {
                app.ui.record = 1;
                app.ui.show_data = true;
            }
        }
        "data.set_serial" => {
            let name = app.session.doc(None).map_or_else(
                || "serial".to_owned(),
                |d| {
                    (1..)
                        .map(|i| {
                            if i == 1 {
                                "serial".to_owned()
                            } else {
                                format!("serial{i}")
                            }
                        })
                        .find(|n| !d.serials.contains_key(n))
                        .unwrap_or_default()
                },
            );
            if app.run_reporting(id, json!({ "name": name })).ok {
                app.ui.show_data = true;
            }
        }
        "element.add_image" => {
            let Some(path) = pick_file(app, &["png", "jpg", "jpeg", "bmp", "webp"]) else {
                return;
            };
            if let Some(mut p) = add_defaults(app, id) {
                p["path"] = json!(path);
                add(app, id, p);
            }
        }
        _ if id.starts_with("element.add_") => {
            if let Some(p) = add_defaults(app, id) {
                add(app, id, p);
            }
        }
        _ => {
            app.run_reporting(id, json!({}));
        }
    }
}

/// Decode the codes in an image and add the first one as a barcode or 2D-code element.
pub fn insert_code_from_image(app: &mut LabelyApp, source: &Value) {
    let env = app.run_reporting("code.decode", source.clone());
    let Some(code) = env
        .result
        .as_ref()
        .and_then(|r| r["codes"].as_array())
        .and_then(|c| c.first())
        .cloned()
    else {
        if env.ok {
            app.set_status(app.i18n.t("status-no-code-found"), true);
        }
        return;
    };
    let id = if code["kind"] == "code2d" {
        "element.add_code2d"
    } else {
        "element.add_barcode"
    };
    if let Some(mut p) = add_defaults(app, id) {
        p["data"] = code["text"].clone();
        p["symbology"] = code["symbology"].clone();
        add(app, id, p);
    }
}

/// Add a text element holding `text`, creating the `serial` counter first if it is used.
pub fn insert_bound_text(app: &mut LabelyApp, text: &str) {
    let needs_serial = text.contains("{{serial}}")
        && app
            .session
            .doc(None)
            .is_some_and(|d| !d.serials.contains_key("serial"));
    if needs_serial && !app.run_reporting("data.set_serial", json!({})).ok {
        return;
    }
    if let Some(mut p) = add_defaults(app, "element.add_text") {
        p["text"] = json!(text);
        add(app, "element.add_text", p);
    }
}

fn add(app: &mut LabelyApp, id: &str, p: Value) {
    let env = app.run_reporting(id, p);
    if env.ok {
        let created = env.created.clone();
        app.select(&created);
    }
}

pub fn save(app: &mut LabelyApp) {
    if app.session.doc_path(None).is_some() {
        app.run_reporting("doc.save", json!({}));
        return;
    }
    if !app.opts.native_dialogs {
        app.set_status(app.i18n.t("status-no-dialogs"), true);
        return;
    }
    let name = app
        .session
        .doc(None)
        .map_or_else(|| "label".to_owned(), |d| d.name.clone());
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Labely", &["labely"])
        .set_file_name(format!("{name}.labely"))
        .save_file()
    {
        let env = app.run_reporting("doc.save", json!({ "path": path }));
        if env.ok {
            app.set_status(app.i18n.t("status-saved"), false);
        }
    }
}

/// Ask for a target file and run `export.png` / `export.pdf` (all records for PDF).
fn export(app: &mut LabelyApp, id: &str) {
    if !app.opts.native_dialogs {
        app.set_status(app.i18n.t("status-no-dialogs"), true);
        return;
    }
    let ext = if id == "export.pdf" { "pdf" } else { "png" };
    let name = app
        .session
        .doc(None)
        .map_or_else(|| "label".to_owned(), |d| d.name.clone());
    let Some(path) = rfd::FileDialog::new()
        .add_filter(ext.to_uppercase(), &[ext])
        .set_file_name(format!("{name}.{ext}"))
        .save_file()
    else {
        return;
    };
    let mut p = json!({ "path": path });
    if id == "export.png" {
        p["record"] = json!(app.ui.record);
    }
    if app.run_reporting(id, p).ok {
        app.set_status(app.i18n.t("status-exported"), false);
    }
}

fn pick_file(app: &mut LabelyApp, exts: &[&str]) -> Option<std::path::PathBuf> {
    if !app.opts.native_dialogs {
        app.set_status(app.i18n.t("status-no-dialogs"), true);
        return None;
    }
    rfd::FileDialog::new().add_filter("", exts).pick_file()
}

pub fn scan(app: &mut LabelyApp) {
    let env = app.run_reporting("device.scan", json!({"timeout_ms": 3000}));
    if let Some(Value::Array(devs)) = env.result.as_ref().and_then(|r| r.get("devices")).cloned() {
        app.ui.scanned = devs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_parse() {
        let s = parse_shortcut("Ctrl+Shift+Z").unwrap();
        assert_eq!(s.logical_key, Key::Z);
        assert!(s.modifiers.command && s.modifiers.shift);
        assert_eq!(parse_shortcut("Delete").unwrap().logical_key, Key::Delete);
        for e in Registry::builtin().entries() {
            if let Some(sc) = e.meta.shortcut {
                assert!(parse_shortcut(sc).is_some(), "{}: {sc}", e.id);
            }
            if let Some(m) = e.meta.menu {
                assert!(MENUS.contains(&m), "{} has unknown menu {m}", e.id);
            }
        }
    }

    #[test]
    fn every_menu_command_has_a_label_in_both_languages() {
        for lang in Lang::ALL {
            let i18n = I18n::new(lang);
            for e in Registry::builtin()
                .entries()
                .iter()
                .filter(|e| e.meta.menu.is_some())
            {
                let key = format!("cmd-{}", e.id.replace(['.', '_'], "-"));
                assert!(i18n.has(&key), "{}: missing {key}", lang.code());
            }
            for m in MENUS {
                assert!(i18n.has(&format!("menu-{m}")));
            }
        }
    }
}
