//! Inspector sections of the redesign (design screens 2 and 5): compact position fields, text
//! style, arrange, and the label tab (size, paper, printer, grid, check). Every change is a
//! `element.set_props` or `doc.set_label` edit, as before.

use egui::{Color32, CornerRadius, Margin, RichText, Sense, Stroke, Ui, vec2};
use labely_core::{Document, Element};
use labely_i18n::Arg;
use serde_json::{Map, Value, json};

use crate::app::LabelyApp;
use crate::prefs::Units;
use crate::{icons, menu, panels, theme};

/// Edits collected while drawing; applied after the frame's widgets.
pub type Edits = Vec<(String, Value)>;

/// Bordered numeric field with a short prefix and a unit; returns the drag response.
pub fn field(
    ui: &mut Ui,
    prefix: &str,
    value: &mut f64,
    units: Option<Units>,
    suffix: &str,
    width: f32,
) -> egui::Response {
    let inner = egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.set_width(width - 22.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let lbl = ui
                    .label(RichText::new(prefix).size(11.5).color(theme::MUTED))
                    .id;
                let v = ui.visuals_mut();
                for w in [
                    &mut v.widgets.inactive,
                    &mut v.widgets.hovered,
                    &mut v.widgets.active,
                ] {
                    w.weak_bg_fill = Color32::TRANSPARENT;
                    w.bg_fill = Color32::TRANSPARENT;
                    w.bg_stroke = Stroke::NONE;
                }
                let drag = match units {
                    Some(u) => panels::length_drag(value, u),
                    None => egui::DragValue::new(value)
                        .speed(0.1)
                        .max_decimals(1)
                        .suffix(suffix.to_owned()),
                };
                ui.add_sized([ui.available_width(), 24.0], drag)
                    .labelled_by(lbl)
            })
            .inner
        });
    inner.inner
}

/// iOS-style switch; returns its response (changed when flipped).
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(34.0, 18.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let value = *on;
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), value, "")
    });
    let t = ui.ctx().animate_bool_responsive(resp.id, value);
    let bg = if value { theme::PRIMARY } else { theme::BORDER };
    ui.painter().rect_filled(rect, CornerRadius::same(9), bg);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    resp
}

/// A label on the left and a switch on the right; true when flipped.
pub fn switch_row(ui: &mut Ui, label: &str, on: &mut bool) -> bool {
    ui.horizontal(|ui| {
        let lbl = ui.label(RichText::new(label).color(theme::TEXT2)).id;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch(ui, on).labelled_by(lbl).changed()
        })
        .inner
    })
    .inner
}

/// Segmented control; returns the picked index when it changes.
pub fn segmented(ui: &mut Ui, items: &[(String, Option<&str>)], selected: usize) -> Option<usize> {
    let mut picked = None;
    egui::Frame::new()
        .fill(theme::BG)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let n = items.len().max(1) as f32;
                let w = ((ui.available_width() - 2.0 * (n - 1.0)) / n).max(28.0);
                for (i, (text, icon)) in items.iter().enumerate() {
                    let active = i == selected;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
                    let label = text.clone();
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(egui::Role::Button, true, active, &label)
                    });
                    if active {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(6), theme::SURFACE);
                    } else if resp.hovered() {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(6), theme::BORDER_SOFT);
                    }
                    let fg = if active { theme::TEXT } else { theme::MUTED };
                    match icon {
                        Some(name) => {
                            icons::paint(
                                ui.painter(),
                                egui::Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
                                name,
                                fg,
                                1.6,
                            );
                            resp.clone().on_hover_text(text);
                        }
                        None => {
                            theme::fit_text(
                                ui.painter(),
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                text,
                                if active {
                                    theme::bold(12.5)
                                } else {
                                    theme::regular(12.5)
                                },
                                fg,
                                rect.width() - 8.0,
                            );
                        }
                    }
                    if resp.clicked() && !active {
                        picked = Some(i);
                    }
                }
            });
        });
    picked
}

fn divider(ui: &mut Ui) {
    ui.add_space(8.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [r.left_center(), r.right_center()],
        Stroke::new(1.0, theme::BORDER_SOFT),
    );
    ui.add_space(4.0);
}

fn num(whole: &Value, key: &str) -> f64 {
    whole.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Name, X / Y / width / height, rotation with quarter turn, flips.
pub fn frame_section(app: &mut LabelyApp, ui: &mut Ui, whole: &Value, edits: &mut Edits) {
    theme::section(ui, &app.i18n.t("props-frame"));
    let units = app.units();
    let gap = 8.0;
    let w = (ui.available_width() - gap) / 2.0;
    let mut name = whole
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-name")).color(theme::TEXT2))
            .id;
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY))
            .labelled_by(lbl)
            .changed()
        {
            edits.push(("name".into(), json!(name)));
        }
    });
    for pair in [["x_mm", "y_mm"], ["width_mm", "height_mm"]] {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for key in pair {
                let mut v = num(whole, key);
                let prefix = app
                    .i18n
                    .t(&format!("field-{}", key.trim_end_matches("_mm")));
                if field(ui, &prefix, &mut v, Some(units), "", w).changed() {
                    edits.push((key.into(), json!(v)));
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut deg = num(whole, "rotation_deg");
        if field(ui, &app.i18n.t("field-rotation"), &mut deg, None, "°", w)
            .on_hover_text(app.i18n.t("prop-rotation-deg"))
            .changed()
        {
            edits.push(("rotation_deg".into(), json!(deg)));
        }
        let turn = app.i18n.t("action-rotate-quarter");
        if icons::button(ui, "rotate", &turn, false).clicked() {
            edits.push(("rotation_deg".into(), json!((deg + 90.0).rem_euclid(360.0))));
        }
        for (key, icon) in [("flip_h", "flip"), ("flip_v", "flip_v")] {
            let on = whole.get(key).and_then(Value::as_bool).unwrap_or(false);
            let tip = app.i18n.t(&format!("prop-{}", key.replace('_', "-")));
            if icons::button(ui, icon, &tip, on).clicked() {
                edits.push((key.into(), json!(!on)));
            }
        }
    });
    divider(ui);
}

/// Font, size, bold / italic / underline, alignment and fit of a text element.
pub fn text_section(app: &mut LabelyApp, ui: &mut Ui, whole: &Value, edits: &mut Edits) {
    theme::section(ui, &app.i18n.t("props-text"));
    let mut names = app.session.services().renderer.fonts();
    let cur = whole
        .get("font_family")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if !cur.is_empty() && !names.contains(&cur) {
        names.insert(0, cur.clone());
    }
    let mut pick = cur.clone();
    let gap = 8.0;
    let full = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-font")).color(theme::TEXT2))
            .id;
        egui::ComboBox::from_id_salt("font")
            .selected_text(cur.clone())
            .width(ui.available_width() - 28.0)
            .show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(&mut pick, n.clone(), n);
                }
            })
            .response
            .labelled_by(lbl);
    });
    if pick != cur {
        edits.push(("font_family".into(), json!(pick)));
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        let mut size = num(whole, "font_size_pt");
        let w = (full - gap) / 2.0;
        if field(ui, &app.i18n.t("field-size"), &mut size, None, " pt", w).changed() {
            edits.push(("font_size_pt".into(), json!(size.max(1.0))));
        }
    });
    ui.horizontal(|ui| {
        let fits = ["none", "shrink", "fill"];
        let fit = whole.get("fit").and_then(Value::as_str).unwrap_or("none");
        let mut pick = fit.to_owned();
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-fit")).color(theme::TEXT2))
            .id;
        egui::ComboBox::from_id_salt("text-fit")
            .selected_text(app.i18n.t(&format!("fit-{fit}")))
            .width(ui.available_width() - 28.0)
            .show_ui(ui, |ui| {
                for f in fits {
                    ui.selectable_value(&mut pick, f.to_owned(), app.i18n.t(&format!("fit-{f}")));
                }
            })
            .response
            .labelled_by(lbl);
        if pick != fit {
            edits.push(("fit".into(), json!(pick)));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        let w = (full - gap) / 2.0;
        ui.allocate_ui(vec2(w, 32.0), |ui| {
            let styles = ["bold", "italic", "underline"];
            let on: Vec<bool> = styles
                .iter()
                .map(|k| whole.get(*k).and_then(Value::as_bool).unwrap_or(false))
                .collect();
            egui::Frame::new()
                .fill(theme::BG)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (i, k) in styles.iter().enumerate() {
                            let tip = app.i18n.t(&format!("prop-{k}"));
                            if icons::button_sized(ui, k, &tip, on[i], 26.0).clicked() {
                                edits.push(((*k).into(), json!(!on[i])));
                            }
                        }
                    });
                });
        });
        let align = whole.get("align").and_then(Value::as_str).unwrap_or("left");
        let order = ["left", "center", "right", "justify"];
        let items: Vec<(String, Option<&str>)> = order
            .iter()
            .zip(["t_left", "t_center", "t_right", "t_justify"])
            .map(|(a, i)| (app.i18n.t(&format!("halign-{a}")), Some(i)))
            .collect();
        let sel = order.iter().position(|a| *a == align).unwrap_or(0);
        if let Some(i) = segmented(ui, &items, sel) {
            edits.push(("align".into(), json!(order[i])));
        }
    });
    divider(ui);
}

const SYMBOLOGIES: &[(&str, &str)] = &[
    ("code128", "Code 128"),
    ("code128_a", "Code 128 A"),
    ("code128_b", "Code 128 B"),
    ("code128_c", "Code 128 C"),
    ("gs1_128", "GS1-128"),
    ("code39", "Code 39"),
    ("code93", "Code 93"),
    ("ean13", "EAN-13"),
    ("ean8", "EAN-8"),
    ("upc_a", "UPC-A"),
    ("upc_e", "UPC-E"),
    ("itf", "ITF"),
    ("codabar", "Codabar"),
];

/// Modulo-10 check digit of EAN/UPC data without its last digit.
fn gtin_check(body: &str) -> Option<u32> {
    let mut sum = 0;
    for (i, c) in body.chars().rev().enumerate() {
        let d = c.to_digit(10)?;
        sum += if i % 2 == 0 { d * 3 } else { d };
    }
    Some((10 - sum % 10) % 10)
}

/// `Some(Ok(check digit))` when EAN/UPC data is complete and valid, `Some(Err(()))` when it
/// is not, `None` for other symbologies or bound data.
fn gtin_status(symbology: &str, data: &str) -> Option<Result<u32, ()>> {
    let len = match symbology {
        "ean13" => 13,
        "ean8" => 8,
        "upc_a" => 12,
        _ => return None,
    };
    if data.contains("{{") {
        return None;
    }
    let n = data.chars().count();
    if n != len || !data.chars().all(|c| c.is_ascii_digit()) {
        return (n != len - 1 || !data.chars().all(|c| c.is_ascii_digit())).then_some(Err(()));
    }
    let (body, last) = data.split_at(len - 1);
    match (
        gtin_check(body),
        last.chars().next().and_then(|c| c.to_digit(10)),
    ) {
        (Some(c), Some(l)) if c == l => Some(Ok(c)),
        _ => Some(Err(())),
    }
}

/// Barcode: type, value with an EAN/UPC check, readable text and its place.
pub fn barcode_section(app: &mut LabelyApp, ui: &mut Ui, whole: &Value, edits: &mut Edits) {
    theme::section(ui, &app.i18n.t("props-barcode"));
    let sym = whole
        .get("symbology")
        .and_then(Value::as_str)
        .unwrap_or("code128")
        .to_owned();
    let shown = SYMBOLOGIES
        .iter()
        .find(|(k, _)| *k == sym)
        .map_or(sym.as_str(), |(_, v)| v)
        .to_owned();
    let mut pick = sym.clone();
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-symbology")).color(theme::TEXT2))
            .id;
        egui::ComboBox::from_id_salt("symbology")
            .selected_text(shown)
            .width(ui.available_width() - 28.0)
            .show_ui(ui, |ui| {
                for (k, v) in SYMBOLOGIES {
                    ui.selectable_value(&mut pick, (*k).to_owned(), *v);
                }
            })
            .response
            .labelled_by(lbl);
    });
    if pick != sym {
        edits.push(("symbology".into(), json!(pick)));
    }
    let data = whole
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let mut text = data.clone();
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-value")).color(theme::TEXT2))
            .id;
        if ui
            .add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY))
            .labelled_by(lbl)
            .changed()
        {
            edits.push(("data".into(), json!(text)));
        }
    });
    match gtin_status(&sym, &data) {
        Some(Ok(c)) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icons::show(ui, "check", 13.0, theme::GREEN);
                ui.label(
                    RichText::new(
                        app.i18n
                            .t_args("barcode-valid", &[("digit", Arg::Num(f64::from(c)))]),
                    )
                    .size(11.5)
                    .color(theme::GREEN),
                );
            });
        }
        Some(Err(())) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icons::show(ui, "warn", 13.0, theme::AMBER);
                ui.label(
                    RichText::new(app.i18n.t("barcode-invalid"))
                        .size(11.5)
                        .color(theme::AMBER),
                );
            });
        }
        None => {}
    }
    let hr = whole
        .get("human_readable")
        .and_then(Value::as_str)
        .unwrap_or("below")
        .to_owned();
    let mut readable = hr != "none";
    if switch_row(ui, &app.i18n.t("prop-readable"), &mut readable) {
        edits.push((
            "human_readable".into(),
            json!(if readable { "below" } else { "none" }),
        ));
    }
    if readable {
        ui.horizontal(|ui| {
            ui.label(RichText::new(app.i18n.t("prop-text-place")).color(theme::TEXT2));
            ui.add_space((ui.available_width() - 150.0).max(0.0));
            let items = [
                (app.i18n.t("place-below"), None),
                (app.i18n.t("place-above"), None),
            ];
            if let Some(i) = segmented(ui, &items, usize::from(hr == "above")) {
                let v = if i == 1 { "above" } else { "below" };
                edits.push(("human_readable".into(), json!(v)));
            }
        });
    }
}

/// Insert a bound value into a text element: date, time or serial number.
pub fn insert_chips(app: &mut LabelyApp, ui: &mut Ui, text: &str, edits: &mut Edits) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (icon, key, value) in [
            ("date", "action-insert-date", "{{now:%Y-%m-%d}}"),
            ("clock", "action-insert-time", "{{now:%H:%M}}"),
            ("serial", "action-insert-serial", "{{serial}}"),
        ] {
            let label = app.i18n.t(key);
            if icons::text_button(ui, icon, &label, None, theme::TEXT2, None)
                .on_hover_text(value)
                .clicked()
            {
                if value == "{{serial}}" {
                    // The counter must exist; the menu action creates it.
                    let doc_has = app
                        .session
                        .doc(None)
                        .is_some_and(|d| d.serials.contains_key("serial"));
                    if !doc_has {
                        app.run_reporting("data.set_serial", json!({}));
                    }
                }
                let sep = if text.is_empty() || text.ends_with(' ') {
                    ""
                } else {
                    " "
                };
                edits.push(("text".into(), json!(format!("{text}{sep}{value}"))));
            }
        }
    });
}

/// Stacking order buttons and the locked / hidden switches.
pub fn arrange_section(app: &mut LabelyApp, ui: &mut Ui, e: &Element, edits: &mut Edits) {
    divider(ui);
    theme::section(ui, &app.i18n.t("props-arrange"));
    let gap = 6.0;
    let w = (ui.available_width() - 3.0 * gap) / 4.0;
    let mut reorder = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (icon, to, key) in [
            ("to_front", "front", "layer-front"),
            ("up", "forward", "layer-forward"),
            ("down", "backward", "layer-backward"),
            ("to_back", "back", "layer-back"),
        ] {
            let text = app.i18n.t(key);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &text));
            ui.painter().rect(
                rect,
                CornerRadius::same(theme::RADIUS),
                if resp.hovered() {
                    theme::BG
                } else {
                    theme::SURFACE
                },
                Stroke::new(1.0, theme::BORDER),
                egui::StrokeKind::Inside,
            );
            icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 16.0), vec2(15.0, 15.0)),
                icon,
                theme::TEXT2,
                1.5,
            );
            let short = app.i18n.t(&format!("{key}-short"));
            theme::fit_text(
                ui.painter(),
                rect.center_bottom() - vec2(0.0, 11.0),
                egui::Align2::CENTER_CENTER,
                short,
                theme::regular(10.5),
                theme::TEXT2,
                rect.width() - 6.0,
            );
            if resp.on_hover_text(&text).clicked() {
                reorder = Some(to);
            }
        }
    });
    if let Some(to) = reorder {
        app.run_reporting("element.reorder", json!({"id": e.id, "to": to}));
    }
    ui.add_space(4.0);
    for key in ["locked", "hidden"] {
        let mut on = if key == "locked" { e.locked } else { e.hidden };
        if switch_row(ui, &app.i18n.t(&format!("prop-{key}")), &mut on) {
            edits.push((key.into(), json!(on)));
        }
    }
}

// ---------- label tab ----------

/// The label tab: size and orientation, paper, printer, grid and snapping, check.
pub fn label_tab(app: &mut LabelyApp, ui: &mut Ui, doc: &Document) {
    let units = app.units();
    let mut edits: Edits = Vec::new();
    let gap = 8.0;
    let w = (ui.available_width() - gap) / 2.0;

    theme::section(ui, &app.i18n.t("props-label"));
    let mut name = doc.name.clone();
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-name")).color(theme::TEXT2))
            .id;
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY))
            .labelled_by(lbl)
            .changed()
        {
            edits.push(("name".into(), json!(name)));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (key, v) in [
            ("width_mm", doc.label.width_mm),
            ("height_mm", doc.label.height_mm),
        ] {
            let mut v = v;
            let prefix = app
                .i18n
                .t(&format!("field-{}", key.trim_end_matches("_mm")));
            if field(ui, &prefix, &mut v, Some(units), "", w).changed() {
                edits.push((key.into(), json!(v)));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        let landscape = doc.label.width_mm >= doc.label.height_mm;
        ui.allocate_ui(vec2(w, 32.0), |ui| {
            let items = [
                (app.i18n.t("orientation-landscape"), None),
                (app.i18n.t("orientation-portrait"), None),
            ];
            if segmented(ui, &items, usize::from(!landscape)).is_some() {
                menu::invoke(app, "doc.rotate_label");
            }
        });
        let mut r = doc.label.corner_radius_mm;
        if field(ui, &app.i18n.t("field-radius"), &mut r, Some(units), "", w)
            .on_hover_text(app.i18n.t("prop-corner-radius-mm"))
            .changed()
        {
            edits.push(("corner_radius_mm".into(), json!(r.max(0.0))));
        }
    });
    divider(ui);

    theme::section(ui, &app.i18n.t("home-paper"));
    let media = serde_json::to_value(doc.label.media)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut pick = media.clone();
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-media")).color(theme::TEXT2))
            .id;
        let w = ui.available_width() - 28.0;
        media_combo(app, ui, "media", &mut pick, w).labelled_by(lbl);
    });
    if pick != media {
        edits.push(("media".into(), json!(pick)));
    }
    label_colors(app, ui);
    divider(ui);

    theme::section(ui, &app.i18n.t("prop-printer-model"));
    let models = app.session.services().devices.models();
    let current = doc.label.printer_model.clone().unwrap_or_default();
    let mut pick = current.clone();
    let shown = models
        .iter()
        .find(|m| m.id == current)
        .map_or_else(|| app.i18n.t("model-none"), |m| m.name.clone());
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("prop-printer-model")).color(theme::TEXT2))
            .id;
        egui::ComboBox::from_id_salt("printer_model")
            .selected_text(shown)
            .width(ui.available_width() - 28.0)
            .show_ui(ui, |ui| {
                for m in &models {
                    ui.selectable_value(
                        &mut pick,
                        m.id.clone(),
                        format!("{} ({} dpi)", m.name, m.dpi),
                    );
                }
            })
            .response
            .labelled_by(lbl);
    });
    if pick != current {
        edits.push(("printer_model".into(), json!(pick)));
    }
    if let Some(m) = models.iter().find(|m| m.id == current) {
        let mut d = doc.label.density.unwrap_or(m.density_default);
        ui.horizontal(|ui| {
            let lbl = ui
                .label(RichText::new(app.i18n.t("prop-density")).color(theme::TEXT2))
                .id;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{d} / {}", m.density_max)).font(theme::bold(12.5)));
                ui.spacing_mut().slider_width = ui.available_width() - 8.0;
                if ui
                    .add(egui::Slider::new(&mut d, m.density_min..=m.density_max).show_value(false))
                    .labelled_by(lbl)
                    .changed()
                {
                    edits.push(("density".into(), json!(d)));
                }
            });
        });
    }
    divider(ui);

    theme::section(ui, &app.i18n.t("props-grid-snap"));
    ui.horizontal(|ui| {
        let lbl = ui
            .label(RichText::new(app.i18n.t("action-show-grid")).color(theme::TEXT2))
            .id;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch(ui, &mut app.ui.show_grid).labelled_by(lbl);
            let mut g = app.ui.grid_mm;
            if field(ui, &app.i18n.t("field-grid"), &mut g, Some(units), "", 84.0)
                .on_hover_text(app.i18n.t("prop-grid-mm"))
                .changed()
            {
                app.ui.grid_mm = g.clamp(0.5, 20.0);
            }
        });
    });
    switch_row(ui, &app.i18n.t("action-snap"), &mut app.ui.snap);
    divider(ui);

    apply_label(app, edits);
    check_cards(app, ui);
}

fn circle_points(c: egui::Pos2, r: f32) -> Vec<egui::Pos2> {
    (0..=32)
        .map(|i| {
            let a = i as f32 / 32.0 * std::f32::consts::TAU;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Paper type combo box.
pub fn media_combo(
    app: &LabelyApp,
    ui: &mut Ui,
    id: &str,
    pick: &mut String,
    width: f32,
) -> egui::Response {
    let shown = app.i18n.t(&format!("media-{}", pick.replace('_', "-")));
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(width)
        .show_ui(ui, |ui| {
            for m in ["gap", "black_mark", "continuous", "transparent"] {
                let text = app.i18n.t(&format!("media-{}", m.replace('_', "-")));
                ui.selectable_value(pick, m.to_owned(), text);
            }
        })
        .response
}

/// Label colour swatches with the current colour's name (a studio preference).
pub fn label_colors(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let mut chosen = None;
        let mut current = "";
        for (key, rgb) in crate::menu::LABEL_COLORS {
            let active = app.ui.label_color == rgb;
            if active {
                current = key;
            }
            let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
            let name = app.i18n.t(key);
            resp.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::RadioButton, true, active, &name)
            });
            let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            if active {
                ui.painter()
                    .circle_stroke(rect.center(), 11.5, Stroke::new(2.0, theme::PRIMARY));
            }
            if key == "color-transparent" {
                ui.painter().add(egui::Shape::dashed_line(
                    &circle_points(rect.center(), 8.5),
                    Stroke::new(1.0, theme::MUTED),
                    2.0,
                    2.0,
                ));
            } else {
                ui.painter()
                    .circle(rect.center(), 8.5, c, Stroke::new(1.0, theme::BORDER));
            }
            if resp.on_hover_text(app.i18n.t(key)).clicked() {
                chosen = Some(rgb);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(app.i18n.t(current))
                    .size(12.0)
                    .color(theme::TEXT2),
            );
        });
        if let Some(rgb) = chosen {
            app.ui.label_color = rgb;
            app.prefs.label_color = Some(rgb);
            app.save_prefs();
        }
    });
}

fn apply_label(app: &mut LabelyApp, edits: Edits) {
    for (key, value) in edits {
        let mut p = Map::new();
        p.insert(key.clone(), value);
        p.insert("coalesce".into(), json!(format!("label-{key}")));
        if app.run_reporting("doc.set_label", Value::Object(p)).ok {
            app.ui.fit_pending |= key.ends_with("_mm");
        }
    }
}

/// Check results as tinted cards, with a re-run link.
pub fn check_cards(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        theme::section(ui, &app.i18n.t("panel-check"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(app.i18n.t("action-run-check"))
                            .size(12.0)
                            .color(theme::PRIMARY),
                    )
                    .frame(false),
                )
                .clicked()
            {
                menu::invoke(app, "doc.check");
                app.ui.show_check = false;
            }
        });
    });
    let Some(check) = app.ui.check.clone() else {
        ui.label(
            RichText::new(app.i18n.t("check-not-run"))
                .size(12.0)
                .color(theme::MUTED),
        );
        return;
    };
    let issues = check["issues"].as_array().cloned().unwrap_or_default();
    if issues.is_empty() {
        card(
            ui,
            theme::GREEN_TINT,
            "check",
            theme::GREEN,
            &app.i18n.t("check-clean"),
            "",
        );
        return;
    }
    for issue in issues {
        let (tint, icon, color) = match issue["severity"].as_str() {
            Some("error") => (Color32::from_rgb(0xFF, 0xF0, 0xF0), "warn", theme::RED),
            Some("warning") => (theme::AMBER_TINT, "warn", theme::AMBER),
            _ => (theme::BG, "info", theme::MUTED),
        };
        let r = card(
            ui,
            tint,
            icon,
            color,
            issue["message"].as_str().unwrap_or(""),
            issue["hint"].as_str().unwrap_or(""),
        );
        if r.clicked()
            && let Some(id) = issue["element_id"].as_u64()
        {
            app.select(&[labely_core::ElementId(id)]);
        }
    }
    let n = check["issues"].as_array().map_or(0, Vec::len);
    ui.label(
        RichText::new(
            app.i18n
                .t_args("check-count", &[("count", Arg::Num(n as f64))]),
        )
        .size(11.5)
        .color(theme::MUTED),
    );
}

fn card(
    ui: &mut Ui,
    fill: Color32,
    icon: &str,
    color: Color32,
    title: &str,
    hint: &str,
) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                icons::show(ui, icon, 16.0, color);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add(
                        egui::Label::new(
                            RichText::new(title)
                                .font(theme::bold(12.5))
                                .color(theme::TEXT),
                        )
                        .wrap(),
                    );
                    if !hint.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(hint).size(11.5).color(theme::TEXT2))
                                .wrap(),
                        );
                    }
                });
            });
        })
        .response
        .interact(Sense::click())
}

/// The text value of an element, for the insert chips.
pub fn text_of(e: &Element) -> Option<String> {
    match &e.kind {
        labely_core::ElementKind::Text(t) => Some(t.text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtin_check_digits() {
        assert_eq!(gtin_status("ean13", "5901234123457"), Some(Ok(7)));
        assert_eq!(gtin_status("ean13", "5901234123458"), Some(Err(())));
        // Without the check digit the renderer adds it.
        assert_eq!(gtin_status("ean13", "590123412345"), None);
        assert_eq!(gtin_status("ean8", "96385074"), Some(Ok(4)));
        assert_eq!(gtin_status("upc_a", "036000291452"), Some(Ok(2)));
        assert_eq!(gtin_status("ean13", "{{EAN}}"), None);
        assert_eq!(gtin_status("code128", "abc"), None);
        assert_eq!(gtin_status("ean13", "12AB"), Some(Err(())));
    }
}
