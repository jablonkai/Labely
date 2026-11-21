//! Side panels: layers, properties (generated from the command schemas), check results,
//! printers, the status bar, the welcome screen and the print-raster preview.

use egui::{Color32, Context, RichText, Ui};
use labely_core::{Element, ElementId, Registry};
use labely_i18n::Arg;
use serde_json::{Map, Value, json};

use crate::app::LabelyApp;
use crate::prefs::Units;
use crate::{menu, theme};

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

pub(crate) fn element_title(app: &LabelyApp, e: &Element) -> String {
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

/// Line style of a `dash_mm` value: solid (none), dashed or dotted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

impl LineStyle {
    pub const ALL: [LineStyle; 3] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Dotted];

    fn key(self) -> &'static str {
        match self {
            LineStyle::Solid => "line-solid",
            LineStyle::Dashed => "line-dashed",
            LineStyle::Dotted => "line-dotted",
        }
    }

    /// Read a `dash_mm` value: a dash no longer than the gap counts as dotted.
    pub fn of(dash: &Value) -> Self {
        match dash.as_array().map(|a| (a.first(), a.get(1))) {
            Some((Some(on), Some(off))) => match (on.as_f64(), off.as_f64()) {
                (Some(on), Some(off)) if on <= off => LineStyle::Dotted,
                _ => LineStyle::Dashed,
            },
            _ => LineStyle::Solid,
        }
    }

    /// The `dash_mm` value for this style; dots and gaps follow the stroke width.
    pub fn dash(self, stroke_mm: f64) -> Value {
        let s = stroke_mm.max(0.1);
        match self {
            LineStyle::Solid => Value::Null,
            LineStyle::Dashed => json!([(s * 6.0).max(1.5), (s * 3.0).max(0.8)]),
            LineStyle::Dotted => json!([s, s * 1.5]),
        }
    }
}

/// Combo box for a line's dash pattern. Returns the new `dash_mm` when changed.
fn dash_editor(app: &LabelyApp, ui: &mut Ui, value: &Value, stroke_mm: f64) -> Option<Value> {
    let lbl = ui.label(app.i18n.t("prop-line-style")).id;
    let cur = LineStyle::of(value);
    let mut pick = cur;
    egui::ComboBox::from_id_salt("line-style")
        .selected_text(app.i18n.t(cur.key()))
        .show_ui(ui, |ui| {
            for s in LineStyle::ALL {
                ui.selectable_value(&mut pick, s, app.i18n.t(s.key()));
            }
        })
        .response
        .labelled_by(lbl);
    ui.end_row();
    (pick != cur).then(|| pick.dash(stroke_mm))
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
/// Inspector body: the label (with its check results) or the selected element.
pub fn properties(app: &mut LabelyApp, ui: &mut Ui, label_tab: bool) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let selection = app.selection();
    egui::ScrollArea::vertical().show(ui, |ui| match selection.as_slice() {
        _ if label_tab => crate::inspector::label_tab(app, ui, &doc),
        [] => crate::inspector::label_tab(app, ui, &doc),
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

    crate::inspector::frame_section(app, ui, &whole, &mut edits);
    let is_text = matches!(e.kind, labely_core::ElementKind::Text(_));
    if is_text {
        crate::inspector::text_section(app, ui, &whole, &mut edits);
    }
    let is_barcode = matches!(e.kind, labely_core::ElementKind::Barcode(_));
    if is_barcode {
        crate::inspector::barcode_section(app, ui, &whole, &mut edits);
    }
    /// Keys the sections above already edit.
    const STYLED: &[&str] = &[
        "font_family",
        "font_size_pt",
        "bold",
        "italic",
        "underline",
        "align",
        "fit",
    ];

    theme::section(ui, &app.i18n.t("props-content"));
    {
        {
            egui::Grid::new("content").num_columns(2).show(ui, |ui| {
                let Some(obj) = props_schema.as_object() else {
                    return;
                };
                for (key, s) in obj {
                    let skip = FRAME_FIELDS.contains(&key.as_str())
                        || (is_text && STYLED.contains(&key.as_str()))
                        || (is_barcode
                            && matches!(key.as_str(), "data" | "symbology" | "human_readable"))
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
                    if key == "dash_mm" {
                        let stroke = whole.get("stroke_mm").and_then(Value::as_f64);
                        if let Some(v) = dash_editor(app, ui, &value, stroke.unwrap_or(0.3)) {
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
                    let tkey = format!("prop-{}", key.replace('_', "-"));
                    let label = if app.i18n.has(&tkey) {
                        app.i18n.t(&tkey)
                    } else {
                        key.strip_suffix("_mm").unwrap_or(key).replace('_', " ")
                    };
                    if let Some(v) = prop_editor(ui, &label, s, &value, units_for(key, app.units()))
                    {
                        edits.push((key.clone(), v));
                    }
                }
            });
        }
    }
    if is_text {
        let text = crate::inspector::text_of(e).unwrap_or_default();
        crate::inspector::insert_chips(app, ui, &text, &mut edits);
    }

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

    crate::inspector::arrange_section(app, ui, e, &mut edits);

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
                if ui
                    .button(app.i18n.t("table-clear"))
                    .on_hover_text(app.i18n.t("table-clear-hint"))
                    .clicked()
                {
                    edits.push(json!({"op": "clear"}));
                }
            });
            cell_style_editor(app, ui, t, (cr, cc), &mut edits);
        });
    app.ui.table_cell = (cr, cc);
    for op in edits {
        // Typing and dragging a size coalesce into one undo step per cell, row or column.
        let coalesce = match op["op"].as_str() {
            Some("set_cell") if op.get("text").is_some() || op.get("font_size_pt").is_some() => {
                Some(format!("cell-{}-{}-{}", id.0, op["row"], op["col"]))
            }
            Some("resize_row") => Some(format!("row-{}-{}", id.0, op["at"])),
            Some("resize_column") => Some(format!("col-{}-{}", id.0, op["at"])),
            _ => None,
        };
        let mut p = json!({"id": id, "edit": op});
        if let Some(c) = coalesce {
            p["coalesce"] = json!(c);
        }
        app.run_reporting("element.table_edit", p);
    }
}

/// Style of the focused cell plus the size of its row and column.
fn cell_style_editor(
    app: &LabelyApp,
    ui: &mut Ui,
    t: &labely_core::document::TableProps,
    (r, c): (usize, usize),
    edits: &mut Vec<Value>,
) {
    use labely_core::document::HAlign;
    let cell = &t.cells[r][c];
    egui::Grid::new("table-cell-style")
        .num_columns(2)
        .show(ui, |ui| {
            let lbl = ui.label(app.i18n.t("table-cell-align")).id;
            let mut align = cell.align;
            let name = |a: Option<HAlign>| match a {
                None => app.i18n.t("table-align-default"),
                Some(a) => app.i18n.t(&format!("halign-{}", halign_key(a))),
            };
            egui::ComboBox::from_id_salt("table-cell-align")
                .selected_text(name(align))
                .show_ui(ui, |ui| {
                    // A cell cannot go back to "inherit" (absent fields stay as they are),
                    // so only concrete alignments are offered once one is set.
                    if cell.align.is_none() {
                        ui.selectable_value(&mut align, None, name(None));
                    }
                    for a in [HAlign::Left, HAlign::Center, HAlign::Right, HAlign::Justify] {
                        ui.selectable_value(&mut align, Some(a), name(Some(a)));
                    }
                })
                .response
                .labelled_by(lbl);
            if let Some(a) = align.filter(|_| align != cell.align) {
                edits.push(json!({"op": "set_cell", "row": r, "col": c, "align": a}));
            }
            ui.end_row();

            let lbl = ui.label(app.i18n.t("table-cell-size")).id;
            let mut size = cell.font_size_pt.unwrap_or(t.font_size_pt);
            if ui
                .add(
                    egui::DragValue::new(&mut size)
                        .range(2.0..=72.0)
                        .speed(0.2)
                        .suffix(" pt"),
                )
                .labelled_by(lbl)
                .changed()
            {
                edits.push(json!({"op": "set_cell", "row": r, "col": c, "font_size_pt": size}));
            }
            ui.end_row();

            let lbl = ui.label(app.i18n.t("table-cell-invert")).id;
            let mut invert = cell.invert;
            if ui.checkbox(&mut invert, "").labelled_by(lbl).changed() {
                edits.push(json!({"op": "set_cell", "row": r, "col": c, "invert": invert}));
            }
            ui.end_row();

            // Row heights and column widths are relative weights.
            let lbl = ui.label(app.i18n.t("table-row-height")).id;
            let mut h = t.row_heights[r];
            if ui
                .add(
                    egui::DragValue::new(&mut h)
                        .range(0.1..=20.0)
                        .speed(0.05)
                        .max_decimals(2),
                )
                .labelled_by(lbl)
                .on_hover_text(app.i18n.t("table-weight-hint"))
                .changed()
            {
                edits.push(json!({"op": "resize_row", "at": r, "height": h}));
            }
            ui.end_row();

            let lbl = ui.label(app.i18n.t("table-column-width")).id;
            let mut w = t.column_widths[c];
            if ui
                .add(
                    egui::DragValue::new(&mut w)
                        .range(0.1..=20.0)
                        .speed(0.05)
                        .max_decimals(2),
                )
                .labelled_by(lbl)
                .on_hover_text(app.i18n.t("table-weight-hint"))
                .changed()
            {
                edits.push(json!({"op": "resize_column", "at": c, "width": w}));
            }
            ui.end_row();
        });
}

/// Message-key suffix of a horizontal alignment.
fn halign_key(a: labely_core::document::HAlign) -> &'static str {
    use labely_core::document::HAlign;
    match a {
        HAlign::Left => "left",
        HAlign::Center => "center",
        HAlign::Right => "right",
        HAlign::Justify => "justify",
    }
}

/// Bottom tools: check results and printers.
pub fn tools(app: &mut LabelyApp, ui: &mut Ui) {
    let both = app.ui.show_check && app.ui.show_printers;
    if both {
        ui.columns(2, |cols| {
            check_panel(app, &mut cols[0]);
            printers_panel(app, &mut cols[1]);
        });
    } else if app.ui.show_check {
        check_panel(app, ui);
    } else {
        printers_panel(app, ui);
    }
}

/// Sheet and header-row controls for a spreadsheet attached in this session.
pub(crate) fn workbook_controls(app: &mut LabelyApp, ui: &mut Ui, doc_id: &str) {
    let Some(wb) = app.ui.workbook.clone().filter(|w| w.doc_id == doc_id) else {
        return;
    };
    let mut sheet = wb.sheet.clone();
    let mut header = wb.header;
    ui.horizontal(|ui| {
        if wb.sheets.len() > 1 {
            let l = ui.label(app.i18n.t("data-sheet")).id;
            egui::ComboBox::from_id_salt("data-sheet")
                .selected_text(sheet.clone().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for s in &wb.sheets {
                        ui.selectable_value(&mut sheet, Some(s.clone()), s);
                    }
                })
                .response
                .labelled_by(l);
        }
        ui.checkbox(&mut header, app.i18n.t("data-header-row"))
            .on_hover_text(app.i18n.t("data-header-row-hint"));
    });
    if sheet != wb.sheet || header != wb.header {
        menu::attach_data(app, "data.attach_xlsx", wb.path, sheet, header);
    }
}

/// A column name that can be dragged onto the canvas.
pub(crate) fn column_source(ui: &mut Ui, column: &str) {
    let id = egui::Id::unique(("column-drag", column));
    ui.dnd_drag_source(id, menu::ColumnDrag(column.to_owned()), |ui| {
        ui.add(egui::Label::new(column).sense(egui::Sense::drag()))
    })
    .response
    .on_hover_cursor(egui::CursorIcon::Grab);
}

/// "+" menu: add a text, barcode or 2D code bound to the column.
pub(crate) fn add_from_column_menu(app: &mut LabelyApp, ui: &mut Ui, column: &str) {
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

pub(crate) fn serial_row(
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

pub(crate) fn check_panel(app: &mut LabelyApp, ui: &mut Ui) {
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

pub(crate) fn printers_panel(app: &mut LabelyApp, ui: &mut Ui) {
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
                // Buttons first from the right, so a long address is cut instead of
                // pushing them out of the panel.
                crate::theme::trailing(ui, |ui| {
                    if ui.small_button(app.i18n.t("action-disconnect")).clicked()
                        && app
                            .run_reporting("device.disconnect", json!({"address": d.address}))
                            .ok
                        && app.ui.detected.as_ref().and_then(|x| x["address"].as_str())
                            == Some(d.address.as_str())
                    {
                        app.ui.detected = None;
                    }
                    if ui
                        .small_button(app.i18n.t("action-printer-settings"))
                        .clicked()
                    {
                        crate::paper::open_device(app, Some(d.address.clone()));
                    }
                    if ui.small_button(app.i18n.t("action-status")).clicked() {
                        menu::refresh_status(app, &d.address);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(format!("🖶 {} — {}", d.name, d.address)).truncate(),
                        );
                    });
                });
                device_details(app, ui, d);
            }
            if let Some(address) = app.ui.connect_failed.clone() {
                connect_help(app, ui, &address);
            }
            crate::paper::banner(app, ui);
            for d in app.ui.scanned.clone() {
                let address = d["address"].as_str().unwrap_or("").to_owned();
                if connected.iter().any(|c| c.address == address) {
                    continue;
                }
                crate::theme::trailing(ui, |ui| {
                    if ui.small_button(app.i18n.t("action-connect")).clicked()
                        && menu::connect(app, &address)
                    {
                        crate::paper::detect_and_apply(app, Some(&address));
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(format!(
                                "{} — {address}",
                                d["name"].as_str().unwrap_or("?")
                            ))
                            .truncate(),
                        );
                    });
                });
            }
            if connected.is_empty() && app.ui.scanned.is_empty() {
                ui.label(RichText::new(app.i18n.t("printers-none")).weak());
            }
            ui.separator();
            crate::jobs::queue_section(app, ui);
        });
}

/// One line of facts about a connected printer: battery, lid, labels left, firmware.
pub fn device_summary(app: &LabelyApp, d: &labely_core::services::DeviceInfo) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = app.ui.device_status.get(&d.address) {
        if let Some(b) = s["battery_percent"].as_u64() {
            out.push(
                app.i18n
                    .t_args("device-battery", &[("percent", Arg::Num(b as f64))]),
            );
        }
        if s["lid_closed"].as_bool() == Some(false) {
            out.push(app.i18n.t("device-lid-open"));
        }
        if s["paper_present"].as_bool() == Some(false) {
            out.push(app.i18n.t("device-no-paper"));
        }
        let paper = &s["paper"];
        if let (Some(total), Some(used)) = (paper["total"].as_u64(), paper["used"].as_u64()) {
            let left = total.saturating_sub(used);
            out.push(app.i18n.t_args(
                "device-labels-left",
                &[
                    ("left", Arg::Num(left as f64)),
                    ("total", Arg::Num(total as f64)),
                ],
            ));
        }
    }
    out
}

fn device_details(app: &LabelyApp, ui: &mut Ui, d: &labely_core::services::DeviceInfo) {
    let mut facts = device_summary(app, d);
    if let Some(fw) = &d.firmware {
        facts.push(
            app.i18n
                .t_args("device-firmware", &[("version", Arg::Str(fw))]),
        );
    }
    if let Some(hw) = &d.hardware {
        facts.push(
            app.i18n
                .t_args("device-hardware", &[("version", Arg::Str(hw))]),
        );
    }
    if let Some(sn) = &d.serial {
        facts.push(
            app.i18n
                .t_args("device-serial", &[("serial", Arg::Str(sn))]),
        );
    }
    if !facts.is_empty() {
        ui.label(RichText::new(facts.join(" · ")).weak());
    }
}

/// Tips after a failed connect, by transport.
pub(crate) fn connect_help(app: &mut LabelyApp, ui: &mut Ui, address: &str) {
    let keys: &[&str] = if address.starts_with("usb:") {
        &[
            "connect-tip-power",
            "connect-tip-cable",
            "connect-tip-other-app",
            "connect-tip-port",
        ]
    } else {
        &[
            "connect-tip-power",
            "connect-tip-other-app",
            "connect-tip-bt-permission",
            "connect-tip-distance",
            "connect-tip-rescan",
        ]
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.colored_label(
                Color32::from_rgb(200, 60, 60),
                app.i18n
                    .t_args("connect-failed", &[("address", Arg::Str(address))]),
            );
            if ui.small_button(app.i18n.t("action-close")).clicked() {
                app.ui.connect_failed = None;
            }
        });
        for k in keys {
            ui.label(format!("• {}", app.i18n.t(k)));
        }
        ui.horizontal(|ui| {
            if ui.button(app.i18n.t("action-retry")).clicked() {
                menu::connect(app, address);
            }
            if ui.button(app.i18n.t("action-scan")).clicked() {
                menu::scan(app);
            }
        });
    });
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
