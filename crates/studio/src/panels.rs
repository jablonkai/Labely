//! Side panels: layers, properties (generated from the command schemas), check results,
//! printers, the status bar, the welcome screen and the print-raster preview.

use egui::{Color32, Context, RichText, Ui};
use labely_core::{Element, ElementId, Registry};
use labely_i18n::Arg;
use serde_json::{Map, Value, json};

use crate::app::LabelyApp;
use crate::menu;
use crate::prefs::Units;

/// Element type → the command that creates it (its params schema describes the props).
fn add_command(kind: &str) -> &'static str {
    match kind {
        "text" => "element.add_text",
        "barcode" => "element.add_barcode",
        "code2d" => "element.add_code2d",
        "image" => "element.add_image",
        "rect" => "element.add_rect",
        "ellipse" => "element.add_ellipse",
        "table" => "element.add_table",
        "icon" => "element.add_icon",
        "border" => "element.add_border",
        _ => "element.add_line",
    }
}

fn element_title(app: &LabelyApp, e: &Element) -> String {
    let kind = e.kind.type_name();
    let base = app.i18n.t(&format!("kind-{kind}"));
    if let Some(n) = e.name.as_deref().filter(|n| !n.is_empty()) {
        return n.to_owned();
    }
    let v = serde_json::to_value(&e.kind).unwrap_or_default();
    let snippet = v
        .get("text")
        .or_else(|| v.get("data"))
        .and_then(Value::as_str)
        .map(|s| {
            s.lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(24)
                .collect::<String>()
        });
    match snippet {
        Some(s) if !s.is_empty() => format!("{base}: {s}"),
        _ => format!("{base} #{}", e.id.0),
    }
}

/// Layers, top of the stack first.
pub fn layers(app: &mut LabelyApp, ui: &mut Ui) {
    ui.heading(app.i18n.t("panel-layers"));
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let selection = app.selection();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for e in doc.elements().collect::<Vec<_>>().into_iter().rev() {
            ui.horizontal(|ui| {
                let eye = if e.hidden { "—" } else { "👁" };
                if ui
                    .small_button(eye)
                    .on_hover_text(app.i18n.t("layer-toggle-visible"))
                    .clicked()
                {
                    app.run_reporting(
                        "element.set_props",
                        json!({"id": e.id, "props": {"hidden": !e.hidden}}),
                    );
                }
                let lock = if e.locked { "🔒" } else { "🔓" };
                if ui
                    .small_button(lock)
                    .on_hover_text(app.i18n.t("layer-toggle-lock"))
                    .clicked()
                {
                    app.run_reporting(
                        "element.set_props",
                        json!({"id": e.id, "props": {"locked": !e.locked}}),
                    );
                }
                let selected = selection.contains(&e.id);
                let r = ui.selectable_label(selected, element_title(app, e));
                if r.clicked() {
                    if ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                        app.run("selection.set", json!({"ids": [e.id], "mode": "toggle"}));
                    } else {
                        app.select(&[e.id]);
                    }
                }
                r.context_menu(|ui| {
                    for (to, key) in [
                        ("front", "layer-front"),
                        ("forward", "layer-forward"),
                        ("backward", "layer-backward"),
                        ("back", "layer-back"),
                    ] {
                        if ui.button(app.i18n.t(key)).clicked() {
                            app.run_reporting("element.reorder", json!({"id": e.id, "to": to}));
                            ui.close();
                        }
                    }
                });
            });
        }
    });
}

/// Enum choices of a schema node: `enum` or `oneOf`/`anyOf` of consts.
fn enum_values(schema: &Value) -> Option<Vec<String>> {
    if let Some(Value::Array(a)) = schema.get("enum") {
        return Some(
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        );
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(Value::Array(alts)) = schema.get(key) {
            let mut out = Vec::new();
            for alt in alts {
                if let Some(c) = alt.get("const").and_then(Value::as_str) {
                    out.push(c.to_owned());
                } else if let Some(vs) = enum_values(alt) {
                    out.extend(vs);
                } else if alt.get("type") == Some(&json!("null")) {
                    continue;
                } else {
                    return None;
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

/// Whether a schema node accepts `null` (an optional property).
fn nullable(schema: &Value) -> bool {
    match schema.get("type") {
        Some(Value::Array(a)) => a.iter().any(|t| t == "null"),
        _ => false,
    }
}

/// Primary JSON type of a schema node, ignoring `null`.
fn schema_type(schema: &Value) -> Option<&str> {
    match schema.get("type") {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).find(|t| *t != "null"),
        _ => None,
    }
}

/// One property editor row. Returns a new value when edited.
/// `units` is set for lengths stored in mm: they are shown and typed in that unit.
fn prop_editor(
    ui: &mut Ui,
    label: &str,
    schema: &Value,
    value: &Value,
    units: Option<Units>,
) -> Option<Value> {
    let lbl = ui
        .label(label)
        .on_hover_text(
            schema
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
        .id;
    let out = if let Some(choices) = enum_values(schema) {
        // Omitted enum props are at their default, which is the first variant.
        let mut cur = value
            .as_str()
            .or_else(|| choices.first().map(String::as_str))
            .unwrap_or("")
            .to_owned();
        let before = cur.clone();
        egui::ComboBox::from_id_salt(label)
            .selected_text(cur.clone())
            .show_ui(ui, |ui| {
                for c in &choices {
                    ui.selectable_value(&mut cur, c.clone(), c);
                }
            })
            .response
            .labelled_by(lbl);
        (cur != before).then(|| json!(cur))
    } else {
        match (schema_type(schema), value) {
            (Some("boolean"), _) => {
                let mut b = value.as_bool().unwrap_or(false);
                ui.checkbox(&mut b, "")
                    .labelled_by(lbl)
                    .changed()
                    .then(|| json!(b))
            }
            (Some("number"), _) => {
                let mut n = value.as_f64().unwrap_or(0.0);
                let nullable = nullable(schema);
                let drag = match units {
                    Some(u) => length_drag(&mut n, u),
                    None => egui::DragValue::new(&mut n).speed(0.1).max_decimals(2),
                };
                let drag = if nullable {
                    drag.range(0.0..=f64::MAX)
                } else {
                    drag
                };
                ui.add(drag)
                    .labelled_by(lbl)
                    .changed()
                    // An optional value at 0 means "not set".
                    .then(|| {
                        if nullable && n <= 0.0 {
                            Value::Null
                        } else {
                            json!(n)
                        }
                    })
            }
            (Some("integer"), _) => {
                let mut n = value.as_i64().unwrap_or(0);
                ui.add(egui::DragValue::new(&mut n))
                    .labelled_by(lbl)
                    .changed()
                    .then(|| json!(n))
            }
            (Some("string"), _) | (None, Value::String(_)) => {
                let mut s = value.as_str().unwrap_or("").to_owned();
                let multiline = label == "text";
                let r = if multiline {
                    ui.add(egui::TextEdit::multiline(&mut s).desired_rows(2))
                } else {
                    ui.text_edit_singleline(&mut s)
                };
                r.labelled_by(lbl).changed().then(|| json!(s))
            }
            _ => {
                ui.label(RichText::new(value.to_string()).weak());
                None
            }
        }
    };
    ui.end_row();
    out
}

/// A drag field for a length kept in mm, shown and typed in `units`.
pub fn length_drag(mm: &mut f64, units: Units) -> egui::DragValue<'_> {
    egui::DragValue::new(mm)
        .speed(0.1)
        .custom_formatter(move |v, _| {
            let s = format!("{:.*}", units.decimals(), units.from_mm(v));
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() || s == "-" {
                "0".to_owned()
            } else {
                s.to_owned()
            }
        })
        .custom_parser(move |t| units.parse(t))
        .suffix(format!(" {}", units.symbol()))
}

/// The unit for a property key: lengths end in `_mm`.
fn units_for(key: &str, units: Units) -> Option<Units> {
    key.ends_with("_mm").then_some(units)
}

const FRAME_FIELDS: &[&str] = &["x_mm", "y_mm", "width_mm", "height_mm", "rotation_deg"];

/// Properties of the single selected element, or of the label when nothing is selected.
pub fn properties(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let selection = app.selection();
    egui::ScrollArea::vertical().show(ui, |ui| match selection.as_slice() {
        [] => label_properties(app, ui, &doc),
        [id] => {
            if let Some(e) = doc.element(*id) {
                element_properties(app, ui, e);
            }
        }
        many => {
            ui.heading(app.i18n.t("panel-properties"));
            ui.label(
                app.i18n
                    .t_args("props-multiple", &[("count", Arg::Num(many.len() as f64))]),
            );
        }
    });
}

fn element_properties(app: &mut LabelyApp, ui: &mut Ui, e: &Element) {
    let kind = e.kind.type_name();
    ui.heading(app.i18n.t(&format!("kind-{kind}")));
    let schema = Registry::builtin()
        .get(add_command(kind))
        .map(|c| c.params_schema())
        .unwrap_or_default();
    let props_schema = schema
        .get("properties")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let whole = serde_json::to_value(e).unwrap_or_default();
    let mut edits: Vec<(String, Value)> = Vec::new();
    let mut text_id = None;

    egui::CollapsingHeader::new(app.i18n.t("props-frame"))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("frame").num_columns(2).show(ui, |ui| {
                let name_schema = json!({"type": "string"});
                let name = whole.get("name").cloned().unwrap_or(json!(""));
                if let Some(v) =
                    prop_editor(ui, &app.i18n.t("prop-name"), &name_schema, &name, None)
                {
                    edits.push(("name".into(), v));
                }
                for f in FRAME_FIELDS {
                    let s = props_schema
                        .get(*f)
                        .cloned()
                        .unwrap_or(json!({"type": "number"}));
                    let label = app.i18n.t(&format!("prop-{}", f.replace('_', "-")));
                    if let Some(v) =
                        prop_editor(ui, &label, &s, &whole[*f], units_for(f, app.units()))
                    {
                        edits.push(((*f).into(), v));
                    }
                }
                for f in ["locked", "hidden", "flip_h", "flip_v"] {
                    let label = app.i18n.t(&format!("prop-{}", f.replace('_', "-")));
                    if let Some(v) =
                        prop_editor(ui, &label, &json!({"type": "boolean"}), &whole[f], None)
                    {
                        edits.push((f.into(), v));
                    }
                }
            });
        });

    egui::CollapsingHeader::new(app.i18n.t("props-content"))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("content").num_columns(2).show(ui, |ui| {
                let Some(obj) = props_schema.as_object() else {
                    return;
                };
                for (key, s) in obj {
                    let skip = FRAME_FIELDS.contains(&key.as_str())
                        || matches!(
                            key.as_str(),
                            "doc_id"
                                | "name"
                                | "path"
                                | "data_base64"
                                | "mime"
                                | "row_count"
                                | "column_count"
                                | "cells"
                        );
                    if skip {
                        continue;
                    }
                    let value = whole.get(key).cloned().unwrap_or(Value::Null);
                    if key == "icon" {
                        let names: Vec<String> = app
                            .session
                            .services()
                            .renderer
                            .icons()
                            .into_iter()
                            .map(|i| i.name)
                            .collect();
                        let s = json!({"enum": names});
                        if let Some(v) = prop_editor(ui, "icon", &s, &value, None) {
                            edits.push((key.clone(), v));
                        }
                        continue;
                    }
                    if key == "font_family" {
                        let mut names = app.session.services().renderer.fonts();
                        if let Some(cur) = value.as_str()
                            && !names.iter().any(|n| n == cur)
                        {
                            names.insert(0, cur.to_owned());
                        }
                        let s = json!({"enum": names});
                        if let Some(v) = prop_editor(ui, "font", &s, &value, None) {
                            edits.push((key.clone(), v));
                        }
                        continue;
                    }
                    if key == "text" && matches!(e.kind, labely_core::ElementKind::Text(_)) {
                        let lbl = ui.label("text").id;
                        let mut s = value.as_str().unwrap_or("").to_owned();
                        let r = ui
                            .add(
                                egui::TextEdit::multiline(&mut s)
                                    .id_salt("prop-text")
                                    .desired_rows(2),
                            )
                            .labelled_by(lbl);
                        text_id = Some(r.id);
                        if r.changed() {
                            edits.push((key.clone(), json!(s)));
                        }
                        ui.end_row();
                        continue;
                    }
                    let label = key.strip_suffix("_mm").unwrap_or(key).replace('_', " ");
                    if let Some(v) = prop_editor(ui, &label, s, &value, units_for(key, app.units()))
                    {
                        edits.push((key.clone(), v));
                    }
                }
            });
        });

    if let labely_core::ElementKind::Table(t) = &e.kind {
        table_editor(app, ui, e.id, t);
    }
    if let labely_core::ElementKind::Text(t) = &e.kind {
        app.ui.text_edit_id = text_id;
        span_tools(app, ui, e.id, t);
    }
    if let labely_core::ElementKind::Image(i) = &e.kind {
        crop_editor(app, ui, e.id, i.crop);
    }
    if let Some(link) = e.mirror {
        ui.horizontal(|ui| {
            ui.label(
                app.i18n
                    .t_args("props-mirror-of", &[("id", Arg::Num(link.of.0 as f64))]),
            );
            if ui.small_button(app.i18n.t("action-unlink")).clicked() {
                app.run_reporting(
                    "element.set_props",
                    json!({"id": e.id, "props": {"mirror": null}}),
                );
            }
        });
    }

    for (key, value) in edits {
        let mut props = Map::new();
        // Keep rich-text styles on the same characters while typing.
        if key == "text"
            && let labely_core::ElementKind::Text(t) = &e.kind
            && !t.spans.is_empty()
            && let Some(new) = value.as_str()
        {
            let spans = crate::rich::shift(&t.spans, &t.text, new);
            props.insert("spans".into(), json!(spans));
        }
        props.insert(key.clone(), value);
        app.run_reporting(
            "element.set_props",
            json!({"id": e.id, "props": props, "coalesce": format!("prop-{}-{key}", e.id.0)}),
        );
    }
}

/// The selected character range of the text property field, if any.
fn text_selection(app: &LabelyApp, ui: &Ui) -> Option<(usize, usize)> {
    let id = app.ui.text_edit_id?;
    let state = egui::text_edit::TextEditState::load(ui.ctx(), id)?;
    let r = state.cursor.char_range()?;
    let (a, b) = (r.primary.index.0, r.secondary.index.0);
    Some((a.min(b), a.max(b)))
}

/// Bold / italic / underline / size buttons that style the selected part of the text.
fn span_tools(
    app: &mut LabelyApp,
    ui: &mut Ui,
    id: ElementId,
    t: &labely_core::document::TextProps,
) {
    use crate::rich::{self, Style};
    let sel = text_selection(app, ui).filter(|(a, b)| a < b);
    let mut new_spans = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(app.i18n.t("props-style-selection"))
            .on_hover_text(app.i18n.t("props-style-selection-hint"));
        ui.add_enabled_ui(sel.is_some(), |ui| {
            let (a, b) = sel.unwrap_or((0, 0));
            for (style, text, base) in [
                (Style::Bold, RichText::new("B").strong(), t.bold),
                (Style::Italic, RichText::new("I").italics(), t.italic),
                (
                    Style::Underline,
                    RichText::new("U").underline(),
                    t.underline,
                ),
            ] {
                let on = sel.is_some() && rich::has_style(&t.spans, base, style, a, b);
                if ui.selectable_label(on, text).clicked() {
                    new_spans = rich::toggle(&t.spans, base, style, a, b);
                }
            }
            for (label, factor) in [("A−", 0.8), ("A+", 1.25)] {
                if ui.small_button(label).clicked() {
                    let pt = ((t.font_size_pt * factor) * 2.0).round() / 2.0;
                    new_spans = rich::set_size(&t.spans, a, b, pt.max(1.0));
                }
            }
        });
        if ui
            .add_enabled(
                !t.spans.is_empty(),
                egui::Button::new(app.i18n.t("action-clear-styles")).small(),
            )
            .clicked()
        {
            let (a, b) = sel.unwrap_or((0, 0));
            new_spans = Some(rich::clear(&t.spans, a, b));
        }
    });
    if let Some(spans) = new_spans {
        app.run_reporting(
            "element.set_props",
            json!({"id": id, "props": {"spans": spans}}),
        );
    }
}

/// Four percentage sliders for the image crop, and a reset button.
fn crop_editor(
    app: &mut LabelyApp,
    ui: &mut Ui,
    id: ElementId,
    crop: Option<labely_core::document::Crop>,
) {
    let c = crop.unwrap_or(labely_core::document::Crop {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    });
    let (mut l, mut t, mut r, mut b) = (
        c.left * 100.0,
        c.top * 100.0,
        (1.0 - c.right) * 100.0,
        (1.0 - c.bottom) * 100.0,
    );
    let mut changed = false;
    egui::CollapsingHeader::new(app.i18n.t("props-crop"))
        .default_open(crop.is_some())
        .show(ui, |ui| {
            egui::Grid::new("crop").num_columns(2).show(ui, |ui| {
                let mut v = [l, r, t, b];
                for (i, key) in ["crop-left", "crop-right", "crop-top", "crop-bottom"]
                    .into_iter()
                    .enumerate()
                {
                    let lbl = ui.label(app.i18n.t(key)).id;
                    // The opposite edge; keep at least 1% of the image.
                    let max = (99.0 - v[i ^ 1]).max(0.0);
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut v[i], 0.0..=max)
                                .suffix(" %")
                                .max_decimals(0),
                        )
                        .labelled_by(lbl)
                        .changed();
                    ui.end_row();
                }
                [l, r, t, b] = v;
            });
            if ui
                .add_enabled(
                    crop.is_some(),
                    egui::Button::new(app.i18n.t("action-reset-crop")),
                )
                .clicked()
            {
                app.run_reporting(
                    "element.set_props",
                    json!({"id": id, "props": {"crop": null}}),
                );
            }
        });
    if changed {
        let crop = if l + t + r + b <= 0.0 {
            Value::Null
        } else {
            json!({"left": l / 100.0, "top": t / 100.0, "right": 1.0 - r / 100.0, "bottom": 1.0 - b / 100.0})
        };
        app.run_reporting(
            "element.set_props",
            json!({"id": id, "props": {"crop": crop}, "coalesce": format!("crop-{}", id.0)}),
        );
    }
}

/// Cell texts as a grid of text fields, plus row/column/merge buttons acting on the focused
/// cell.
fn table_editor(
    app: &mut LabelyApp,
    ui: &mut Ui,
    id: ElementId,
    t: &labely_core::document::TableProps,
) {
    let (rows, cols) = (t.row_heights.len(), t.column_widths.len());
    let (mut cr, mut cc) = app.ui.table_cell;
    cr = cr.min(rows - 1);
    cc = cc.min(cols - 1);
    let mut edits: Vec<Value> = Vec::new();
    egui::CollapsingHeader::new(app.i18n.t("props-table"))
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("table").show(ui, |ui| {
                egui::Grid::new("table-cells").show(ui, |ui| {
                    for (r, row) in t.cells.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate() {
                            let covered = t.merge_at(r, c).is_some_and(|m| (m.row, m.col) != (r, c));
                            let mut s = cell.text.clone();
                            let resp = ui.add_enabled(
                                !covered,
                                egui::TextEdit::singleline(&mut s)
                                    .desired_width(56.0)
                                    .hint_text(format!("{}{}", crate::app::column_name(c), r + 1)),
                            );
                            let resp = resp.on_hover_text(format!(
                                "{} {}, {} {}",
                                app.i18n.t("table-row"),
                                r + 1,
                                app.i18n.t("table-column"),
                                c + 1
                            ));
                            if resp.has_focus() || resp.clicked() {
                                (cr, cc) = (r, c);
                            }
                            if resp.changed() {
                                edits.push(json!({"op": "set_cell", "row": r, "col": c, "text": s}));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
            ui.label(
                RichText::new(format!(
                    "{} {}, {} {}",
                    app.i18n.t("table-row"),
                    cr + 1,
                    app.i18n.t("table-column"),
                    cc + 1
                ))
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                let merged = t.merge_at(cr, cc).copied();
                let buttons: [(&str, bool, Value); 8] = [
                    ("table-insert-row", rows < 64, json!({"op": "insert_row", "at": cr + 1})),
                    ("table-delete-row", rows > 1, json!({"op": "delete_row", "at": cr})),
                    ("table-insert-column", cols < 64, json!({"op": "insert_column", "at": cc + 1})),
                    ("table-delete-column", cols > 1, json!({"op": "delete_column", "at": cc})),
                    (
                        "table-merge-right",
                        cc + 1 < cols,
                        json!({"op": "merge", "row": cr, "col": cc, "rows": merged.map_or(1, |m| m.rows), "cols": merged.map_or(1, |m| m.cols) + 1}),
                    ),
                    (
                        "table-merge-down",
                        cr + 1 < rows,
                        json!({"op": "merge", "row": cr, "col": cc, "rows": merged.map_or(1, |m| m.rows) + 1, "cols": merged.map_or(1, |m| m.cols)}),
                    ),
                    ("table-split", merged.is_some(), json!({"op": "split", "row": cr, "col": cc})),
                    (
                        "table-bold",
                        true,
                        json!({"op": "set_cell", "row": cr, "col": cc, "bold": !t.cells[cr][cc].bold}),
                    ),
                ];
                for (key, enabled, op) in buttons {
                    if ui
                        .add_enabled(enabled, egui::Button::new(app.i18n.t(key)).small())
                        .clicked()
                    {
                        edits.push(op);
                    }
                }
            });
        });
    app.ui.table_cell = (cr, cc);
    for op in edits {
        let coalesce = (op["op"] == "set_cell" && op.get("text").is_some())
            .then(|| format!("cell-{}-{}-{}", id.0, op["row"], op["col"]));
        let mut p = json!({"id": id, "edit": op});
        if let Some(c) = coalesce {
            p["coalesce"] = json!(c);
        }
        app.run_reporting("element.table_edit", p);
    }
}

fn label_properties(app: &mut LabelyApp, ui: &mut Ui, doc: &labely_core::Document) {
    ui.heading(app.i18n.t("props-label"));
    let schema = Registry::builtin()
        .get("doc.set_label")
        .map(|c| c.params_schema())
        .unwrap_or_default();
    let props = schema.get("properties").cloned().unwrap_or_default();
    let label = serde_json::to_value(&doc.label).unwrap_or_default();
    let models = app.session.services().devices.models();
    let mut edits: Vec<(String, Value)> = Vec::new();
    egui::Grid::new("label").num_columns(2).show(ui, |ui| {
        if let Some(v) = prop_editor(
            ui,
            &app.i18n.t("prop-name"),
            &json!({"type": "string"}),
            &json!(doc.name),
            None,
        ) {
            edits.push(("name".into(), v));
        }
        for f in [
            "width_mm",
            "height_mm",
            "corner_radius_mm",
            "media",
            "density",
        ] {
            let s = props.get(f).cloned().unwrap_or(json!({"type": "number"}));
            let text = app.i18n.t(&format!("prop-{}", f.replace('_', "-")));
            if let Some(v) = prop_editor(ui, &text, &s, &label[f], units_for(f, app.units())) {
                edits.push((f.into(), v));
            }
        }
        let lbl = ui.label(app.i18n.t("prop-printer-model")).id;
        let current = doc.label.printer_model.clone().unwrap_or_default();
        let mut pick = current.clone();
        let shown = models
            .iter()
            .find(|m| m.id == current)
            .map_or_else(|| app.i18n.t("model-none"), |m| m.name.clone());
        egui::ComboBox::from_id_salt("printer_model")
            .selected_text(shown)
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
        if pick != current {
            edits.push(("printer_model".into(), json!(pick)));
        }
        ui.end_row();
    });
    for (key, value) in edits {
        let mut p = Map::new();
        p.insert(key.clone(), value);
        p.insert("coalesce".into(), json!(format!("label-{key}")));
        if app.run_reporting("doc.set_label", Value::Object(p)).ok {
            app.ui.fit_pending |= key.ends_with("_mm");
        }
    }
}

/// Bottom tools: check results and printers.
pub fn tools(app: &mut LabelyApp, ui: &mut Ui) {
    ui.columns(
        usize::from(app.ui.show_check)
            + usize::from(app.ui.show_printers)
            + usize::from(app.ui.show_data),
        |cols| {
            let mut i = 0;
            if app.ui.show_data {
                data_panel(app, &mut cols[i]);
                i += 1;
            }
            if app.ui.show_check {
                check_panel(app, &mut cols[i]);
                i += 1;
            }
            if app.ui.show_printers {
                printers_panel(app, &mut cols[i]);
            }
        },
    );
}

/// Data source, record navigator, column binding and serial counters.
fn data_panel(app: &mut LabelyApp, ui: &mut Ui) {
    ui.heading(app.i18n.t("panel-data"));
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let bindable = match app.selection().as_slice() {
        [id] => doc.element(*id).is_some_and(|e| {
            matches!(
                e.kind,
                labely_core::ElementKind::Text(_)
                    | labely_core::ElementKind::Barcode(_)
                    | labely_core::ElementKind::Code2d(_)
            )
        }),
        _ => false,
    };
    let values = crate::canvas::record_values(&doc, app.ui.record);
    egui::ScrollArea::vertical().id_salt("data").show(ui, |ui| {
        match &doc.data {
            None => {
                ui.label(app.i18n.t("data-none"));
                ui.horizontal(|ui| {
                    for id in ["data.attach_csv", "data.attach_xlsx"] {
                        if ui.button(menu::command_label(&app.i18n, id)).clicked() {
                            menu::invoke(app, id);
                        }
                    }
                });
            }
            Some(d) => {
                let n = d.rows.len();
                ui.horizontal(|ui| {
                    if let Some(o) = &d.origin {
                        ui.strong(o);
                    }
                    ui.label(
                        app.i18n
                            .t_args("data-records", &[("count", Arg::Num(n as f64))]),
                    );
                    if ui
                        .small_button(menu::command_label(&app.i18n, "data.detach"))
                        .clicked()
                    {
                        app.run_reporting("data.detach", json!({}));
                    }
                });
                if n > 0 {
                    let mut record = app.ui.record.clamp(1, n);
                    ui.horizontal(|ui| {
                        let l = ui.label(app.i18n.t("data-record")).id;
                        if ui.small_button("◀").clicked() {
                            record = record.saturating_sub(1).max(1);
                        }
                        ui.add(egui::DragValue::new(&mut record).range(1..=n))
                            .labelled_by(l);
                        if ui.small_button("▶").clicked() {
                            record = (record + 1).min(n);
                        }
                    });
                    app.ui.record = record;
                }
                ui.weak(app.i18n.t("data-drag-hint"));
                egui::Grid::new("data-columns")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        for c in &d.columns {
                            column_source(ui, c);
                            ui.weak(values.get(c).map_or("", String::as_str));
                            add_from_column_menu(app, ui, c);
                            if ui
                                .add_enabled(
                                    bindable,
                                    egui::Button::new(app.i18n.t("action-bind")).small(),
                                )
                                .on_hover_text(app.i18n.t("action-bind-hint"))
                                .clicked()
                                && let [id] = app.selection().as_slice()
                            {
                                app.run_reporting("data.bind", json!({"id": id, "column": c}));
                            }
                            ui.end_row();
                        }
                    });
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong(app.i18n.t("data-serials"));
            if ui
                .small_button(menu::command_label(&app.i18n, "data.set_serial"))
                .clicked()
            {
                menu::invoke(app, "data.set_serial");
            }
        });
        for (name, spec) in &doc.serials {
            serial_row(app, ui, name, spec, bindable, &values);
        }
    });
}

/// A column name that can be dragged onto the canvas.
fn column_source(ui: &mut Ui, column: &str) {
    let id = egui::Id::unique(("column-drag", column));
    ui.dnd_drag_source(id, menu::ColumnDrag(column.to_owned()), |ui| {
        ui.add(egui::Label::new(column).sense(egui::Sense::drag()))
    })
    .response
    .on_hover_cursor(egui::CursorIcon::Grab);
}

/// "+" menu: add a text, barcode or 2D code bound to the column.
fn add_from_column_menu(app: &mut LabelyApp, ui: &mut Ui, column: &str) {
    let has_doc = app.session.active_doc_id().is_some();
    ui.add_enabled_ui(has_doc, |ui| {
        ui.menu_button("+", |ui| {
            for (key, id, _) in menu::FROM_COLUMN {
                if ui.button(app.i18n.t(key)).clicked() {
                    menu::add_from_column(app, id, column, None);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(app.i18n.t("action-add-from-column"));
    });
}

fn serial_row(
    app: &mut LabelyApp,
    ui: &mut Ui,
    name: &str,
    spec: &labely_core::document::SerialSpec,
    bindable: bool,
    values: &std::collections::BTreeMap<String, String>,
) {
    let mut s = spec.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{{{{{name}}}}}"));
        ui.weak(values.get(name).map_or("", String::as_str));
        if ui
            .add_enabled(
                bindable,
                egui::Button::new(app.i18n.t("action-bind")).small(),
            )
            .on_hover_text(app.i18n.t("action-bind-hint"))
            .clicked()
            && let [id] = app.selection().as_slice()
        {
            app.run_reporting("data.bind", json!({"id": id, "column": name}));
        }
        if ui.small_button("✖").clicked() {
            app.run_reporting("data.remove_serial", json!({ "name": name }));
            return;
        }
        ui.end_row();
        let l = ui.label(app.i18n.t("serial-start")).id;
        ui.add(egui::DragValue::new(&mut s.start)).labelled_by(l);
        let l = ui.label(app.i18n.t("serial-step")).id;
        ui.add(egui::DragValue::new(&mut s.step)).labelled_by(l);
        let l = ui.label(app.i18n.t("serial-digits")).id;
        ui.add(egui::DragValue::new(&mut s.digits).range(0..=18))
            .labelled_by(l);
        let l = ui.label(app.i18n.t("serial-prefix")).id;
        ui.add(egui::TextEdit::singleline(&mut s.prefix).desired_width(48.0))
            .labelled_by(l);
        let l = ui.label(app.i18n.t("serial-suffix")).id;
        ui.add(egui::TextEdit::singleline(&mut s.suffix).desired_width(48.0))
            .labelled_by(l);
    });
    if &s != spec {
        app.run_reporting(
            "data.set_serial",
            json!({
                "name": name, "start": s.start, "step": s.step, "digits": s.digits,
                "prefix": s.prefix, "suffix": s.suffix,
            }),
        );
    }
}

fn check_panel(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading(app.i18n.t("panel-check"));
        if ui.button(app.i18n.t("action-run-check")).clicked() {
            menu::invoke(app, "doc.check");
        }
    });
    let Some(check) = app.ui.check.clone() else {
        ui.label(app.i18n.t("check-not-run"));
        return;
    };
    let issues = check["issues"].as_array().cloned().unwrap_or_default();
    if issues.is_empty() {
        ui.colored_label(Color32::from_rgb(40, 150, 70), app.i18n.t("check-clean"));
    }
    egui::ScrollArea::vertical()
        .id_salt("check")
        .show(ui, |ui| {
            for issue in issues {
                let (sym, color) = match issue["severity"].as_str() {
                    Some("error") => ("⛔", Color32::from_rgb(200, 50, 50)),
                    Some("warning") => ("⚠", Color32::from_rgb(200, 140, 0)),
                    _ => ("ℹ", ui.visuals().weak_text_color()),
                };
                let text = format!(
                    "{sym} {}: {}",
                    issue["rule"].as_str().unwrap_or(""),
                    issue["message"].as_str().unwrap_or("")
                );
                let r = ui
                    .add(
                        egui::Label::new(RichText::new(text).color(color))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_text(issue["hint"].as_str().unwrap_or(""));
                if r.clicked()
                    && let Some(id) = issue["element_id"].as_u64()
                {
                    app.select(&[ElementId(id)]);
                }
            }
        });
}

fn printers_panel(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading(app.i18n.t("panel-printers"));
        if ui.button(app.i18n.t("action-scan")).clicked() {
            menu::scan(app);
        }
    });
    let connected = app.session.services().devices.connected();
    egui::ScrollArea::vertical()
        .id_salt("printers")
        .show(ui, |ui| {
            for d in &connected {
                ui.horizontal(|ui| {
                    ui.label(format!("🖶 {} — {}", d.name, d.address));
                    if ui.small_button(app.i18n.t("action-status")).clicked() {
                        let env = app.run_reporting("device.status", json!({"address": d.address}));
                        if let Some(r) = env.result {
                            app.set_status(r["status"].to_string(), false);
                        }
                    }
                    if ui
                        .small_button(app.i18n.t("action-printer-settings"))
                        .clicked()
                    {
                        crate::paper::open_device(app, Some(d.address.clone()));
                    }
                    if ui.small_button(app.i18n.t("action-disconnect")).clicked()
                        && app
                            .run_reporting("device.disconnect", json!({"address": d.address}))
                            .ok
                        && app.ui.detected.as_ref().and_then(|x| x["address"].as_str())
                            == Some(d.address.as_str())
                    {
                        app.ui.detected = None;
                    }
                });
            }
            crate::paper::banner(app, ui);
            for d in app.ui.scanned.clone() {
                let address = d["address"].as_str().unwrap_or("").to_owned();
                if connected.iter().any(|c| c.address == address) {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(format!("{} — {address}", d["name"].as_str().unwrap_or("?")));
                    if ui.small_button(app.i18n.t("action-connect")).clicked()
                        && app
                            .run_reporting("device.connect", json!({"address": address}))
                            .ok
                    {
                        crate::paper::detect(app, Some(&address));
                    }
                });
            }
            if connected.is_empty() && app.ui.scanned.is_empty() {
                ui.label(RichText::new(app.i18n.t("printers-none")).weak());
            }
            ui.separator();
            crate::jobs::queue_section(app, ui);
        });
}

/// Status bar: document, zoom, MCP state, last message.
pub fn status_bar(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if let Some(doc) = app.session.doc(None) {
            let n = doc.elements().count();
            ui.label(
                app.i18n
                    .t_args("status-elements", &[("count", Arg::Num(n as f64))]),
            );
            ui.separator();
            ui.label(format!("{:.0}%", app.ui.zoom / 8.0 * 100.0));
            ui.separator();
        }
        match &app.mcp {
            Some(h) => {
                ui.label(format!("MCP • {}", h.url.trim_start_matches("http://")))
                    .on_hover_text(app.i18n.t("mcp-on-hint"));
                ui.checkbox(
                    &mut app.ui.allow_agent_print,
                    app.i18n.t("action-allow-agent-print"),
                );
                if let Some(last) = app.ui.agent_log.back() {
                    ui.label(RichText::new(format!("⚙ {last}")).weak());
                }
            }
            None => {
                ui.label(RichText::new(app.i18n.t("mcp-off")).weak());
            }
        }
        if let Some(s) = &app.ui.status
            && s.at.elapsed().as_secs() < 8
        {
            ui.separator();
            let color = if s.error {
                Color32::from_rgb(200, 60, 60)
            } else {
                ui.visuals().text_color()
            };
            ui.colored_label(color, &s.text);
        }
    });
}

/// Shown when no document is open.
pub fn welcome(app: &mut LabelyApp, ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        ui.heading("Labely");
        ui.label(app.i18n.t("welcome-text"));
        ui.add_space(12.0);
        if ui
            .button(menu::command_label(&app.i18n, "doc.new"))
            .clicked()
        {
            menu::invoke(app, "doc.new");
        }
        if ui
            .button(menu::command_label(&app.i18n, "template.new"))
            .clicked()
        {
            menu::invoke(app, "template.new");
        }
        if ui
            .button(menu::command_label(&app.i18n, "doc.open"))
            .clicked()
        {
            menu::invoke(app, "doc.open");
        }
        if !app.prefs.recent.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new(app.i18n.t("menu-recent")).strong());
            let mut open = None;
            for path in &app.prefs.recent {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                if ui
                    .link(name)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    open = Some(path.clone());
                }
            }
            if let Some(path) = open {
                app.open_recent(&path);
            }
        }
    });
}

/// One tab per open document: click to switch, × to close, + for a new label.
pub fn doc_tabs(app: &mut LabelyApp, ui: &mut Ui) {
    let docs = app
        .run("doc.list", json!({}))
        .result
        .and_then(|r| r.get("docs").cloned())
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default();
    let mut activate = None;
    let mut close = None;
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            for d in &docs {
                let id = d["doc_id"].as_str().unwrap_or_default().to_owned();
                let name = d["name"].as_str().unwrap_or_default();
                let dirty = if d["dirty"].as_bool() == Some(true) {
                    " •"
                } else {
                    ""
                };
                let active = d["active"].as_bool() == Some(true);
                let tab = ui
                    .selectable_label(active, format!("{name}{dirty}"))
                    .on_hover_text(d["path"].as_str().unwrap_or(name));
                if tab.clicked() {
                    activate = Some(id.clone());
                }
                if tab.middle_clicked() {
                    close = Some(id.clone());
                }
                if ui
                    .small_button("×")
                    .on_hover_text(app.i18n.t("cmd-doc-close"))
                    .clicked()
                {
                    close = Some(id);
                }
                ui.separator();
            }
            if ui
                .small_button("+")
                .on_hover_text(menu::command_label(&app.i18n, "doc.new"))
                .clicked()
            {
                menu::invoke(app, "doc.new");
            }
        });
    });
    if let Some(id) = activate {
        app.activate(&id);
    }
    if let Some(id) = close {
        app.request_close(&id);
    }
}

/// The print raster at one screen point per dot.
pub fn preview_window(app: &mut LabelyApp, ctx: &Context) {
    let mut open = app.ui.show_preview;
    egui::Window::new(app.i18n.t("panel-preview"))
        .open(&mut open)
        .resizable(true)
        .show(ctx, |ui| {
            if let Some(r) = &app.raster {
                ui.label(format!("{}×{} dots, {} dpi", r.size[0], r.size[1], r.dpi));
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.image((
                        r.texture.id(),
                        egui::vec2(r.size[0] as f32, r.size[1] as f32),
                    ));
                });
            }
        });
    app.ui.show_preview = open;
}
