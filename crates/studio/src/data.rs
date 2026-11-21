//! Data pane of the left rail and the records table under the canvas (design screen 4).
//! Binding, serial numbers and record paging run the same `data.*` commands as before.

use egui::{Color32, CornerRadius, Margin, RichText, Sense, Stroke, Ui, vec2};
use labely_core::{Document, ElementKind};
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::LabelyApp;
use crate::{icons, menu, panels, theme};

/// Whether the single selected element can be bound to a column.
fn bindable(app: &LabelyApp, doc: &Document) -> bool {
    match app.selection().as_slice() {
        [id] => doc.element(*id).is_some_and(|e| {
            matches!(
                e.kind,
                ElementKind::Text(_) | ElementKind::Barcode(_) | ElementKind::Code2d(_)
            )
        }),
        _ => false,
    }
}

/// Whether any element uses `{{column}}`.
fn is_bound(doc: &Document, column: &str) -> bool {
    let needle = format!("{{{{{column}}}}}");
    doc.elements().any(|e| {
        serde_json::to_value(&e.kind).is_ok_and(|v| {
            ["text", "data"]
                .iter()
                .filter_map(|k| v.get(*k).and_then(Value::as_str))
                .any(|s| s.contains(&needle))
        })
    })
}

/// Green "Bound" badge, or the "Bind" button.
fn bind_chip(app: &mut LabelyApp, ui: &mut Ui, column: &str, bound: bool, can_bind: bool) {
    if bound && !can_bind {
        egui::Frame::new()
            .fill(theme::pal().green_tint)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(8, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    icons::show(ui, "check", 12.0, theme::pal().green);
                    ui.label(
                        RichText::new(app.i18n.t("data-bound"))
                            .size(11.5)
                            .color(theme::pal().green),
                    );
                });
            });
        return;
    }
    let r = ui
        .add_enabled(
            can_bind,
            egui::Button::new(
                RichText::new(app.i18n.t("action-bind"))
                    .size(11.5)
                    .color(theme::pal().primary),
            )
            .fill(theme::pal().surface)
            .stroke(Stroke::new(1.0, theme::pal().primary_tint2))
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 22.0)),
        )
        .on_hover_text(app.i18n.t("action-bind-hint"))
        .on_disabled_hover_text(app.i18n.t("action-bind-hint"));
    if r.clicked()
        && let [id] = app.selection().as_slice()
    {
        app.run_reporting("data.bind", json!({"id": id, "column": column}));
    }
}

/// The Data pane: source file, columns with their binding, serial numbers.
pub fn data_pane(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(app.i18n.t("panel-data"))
                .font(theme::bold(16.0))
                .color(theme::pal().text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(RichText::new("···").color(theme::pal().muted), |ui| {
                for id in [
                    "data.attach_csv",
                    "data.attach_xlsx",
                    "data.attach_images",
                    "data.set_serial",
                ] {
                    if ui.button(menu::command_label(&app.i18n, id)).clicked() {
                        menu::invoke(app, id);
                        ui.close();
                    }
                }
            });
        });
    });
    ui.add_space(6.0);
    let can_bind = bindable(app, &doc);
    let values = crate::canvas::record_values(&doc, app.ui.record);
    egui::ScrollArea::vertical().id_salt("data").show(ui, |ui| {
        match &doc.data {
            None => {
                ui.label(RichText::new(app.i18n.t("data-none")).color(theme::pal().text2));
                ui.add_space(6.0);
                for (id, icon) in [("data.attach_csv", "file"), ("data.attach_xlsx", "table")] {
                    let w = ui.available_width();
                    if icons::text_button(
                        ui,
                        icon,
                        &menu::command_label(&app.i18n, id),
                        None,
                        theme::pal().text,
                        Some(w),
                    )
                    .clicked()
                    {
                        menu::invoke(app, id);
                    }
                }
            }
            Some(d) => {
                let n = d.rows.len();
                theme::card().inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                        ui.painter().rect_filled(
                            r,
                            CornerRadius::same(theme::RADIUS),
                            theme::pal().green_tint,
                        );
                        icons::paint(ui.painter(), r.shrink(8.0), "file", theme::pal().green, 1.5);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let name = d.origin.clone().unwrap_or_else(|| app.i18n.t("panel-data"));
                            ui.add(
                                egui::Label::new(RichText::new(name).font(theme::bold(13.0)))
                                    .truncate(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    app.i18n
                                        .t_args("data-records", &[("count", Arg::Num(n as f64))]),
                                    app.i18n.t_args(
                                        "data-columns",
                                        &[("count", Arg::Num(d.columns.len() as f64))]
                                    )
                                ))
                                .size(11.5)
                                .color(theme::pal().muted),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tip = menu::command_label(&app.i18n, "data.detach");
                            if icons::button_sized(ui, "close", &tip, false, 26.0).clicked() {
                                app.run_reporting("data.detach", json!({}));
                            }
                        });
                    });
                });
                if let Some(doc_id) = app.session.active_doc_id().map(str::to_owned) {
                    panels::workbook_controls(app, ui, &doc_id);
                }
                ui.label(
                    RichText::new(app.i18n.t("data-drag-hint"))
                        .size(11.5)
                        .color(theme::pal().muted),
                );
                ui.add_space(4.0);
                theme::section(ui, &app.i18n.t("data-columns-title"));
                for c in &d.columns {
                    let bound = is_bound(&doc, c);
                    egui::Frame::new()
                        .fill(if bound {
                            theme::pal().bg
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(CornerRadius::same(theme::RADIUS))
                        .inner_margin(Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                icons::show(
                                    ui,
                                    if bound { "bind" } else { "data" },
                                    15.0,
                                    if bound {
                                        theme::pal().primary
                                    } else {
                                        theme::pal().muted
                                    },
                                );
                                let text_w = ui.available_width() - 96.0;
                                ui.vertical(|ui| {
                                    ui.set_max_width(text_w);
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    panels::column_source(ui, c);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(values.get(c).map_or("", String::as_str))
                                                .size(11.5)
                                                .color(theme::pal().muted),
                                        )
                                        .truncate(),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        panels::add_from_column_menu(app, ui, c);
                                        bind_chip(app, ui, c, bound, can_bind);
                                    },
                                );
                            });
                        });
                }
            }
        }
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            theme::section(ui, &app.i18n.t("data-serials"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = menu::command_label(&app.i18n, "data.set_serial");
                if icons::button_sized(ui, "plus", &tip, false, 24.0).clicked() {
                    menu::invoke(app, "data.set_serial");
                }
            });
        });
        for (name, spec) in &doc.serials {
            panels::serial_row(app, ui, name, spec, can_bind, &values);
        }
    });
}

/// The records table under the canvas: paging, the print range, one row per record.
pub fn records_table(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let Some(d) = &doc.data else {
        return;
    };
    let n = d.rows.len();
    let mut record = app.ui.record.clamp(1, n.max(1));
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(app.i18n.t("data-records-title"))
                .font(theme::bold(13.5))
                .color(theme::pal().text),
        );
        egui::Frame::new()
            .stroke(Stroke::new(1.0, theme::pal().border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(8, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        app.i18n
                            .t_args("data-records", &[("count", Arg::Num(n as f64))]),
                    )
                    .size(11.5)
                    .color(theme::pal().text2),
                );
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut range = app.ui.print.records.clone();
            let lbl = app.i18n.t("data-print-range");
            ui.add(
                egui::TextEdit::singleline(&mut range)
                    .hint_text(format!("1-{n}"))
                    .desired_width(90.0),
            )
            .on_hover_text(app.i18n.t("print-records-hint"));
            ui.label(RichText::new(lbl).size(12.0).color(theme::pal().text2));
            app.ui.print.records = range;
            ui.add_space(12.0);
            if icons::button_sized(
                ui,
                "chev_right",
                &app.i18n.t("data-record-next"),
                false,
                26.0,
            )
            .clicked()
            {
                record = (record + 1).min(n.max(1));
            }
            let l = ui
                .label(RichText::new(format!(" / {n}")).font(theme::bold(12.5)))
                .id;
            ui.add(egui::DragValue::new(&mut record).range(1..=n.max(1)))
                .labelled_by(l)
                .on_hover_text(app.i18n.t("data-record"));
            if icons::button_sized(
                ui,
                "chev_left",
                &app.i18n.t("data-record-prev"),
                false,
                26.0,
            )
            .clicked()
            {
                record = record.saturating_sub(1).max(1);
            }
        });
    });
    ui.add_space(4.0);
    let cols = d.columns.len();
    let width = ui.available_width();
    let num_w = 36.0;
    let col_w = ((width - num_w) / cols.max(1) as f32).max(90.0);
    let row_h = 28.0;
    let header = |ui: &mut Ui| {
        let (rect, _) =
            ui.allocate_exact_size(vec2(num_w + col_w * cols as f32, row_h), Sense::hover());
        let p = ui.painter();
        p.text(
            rect.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "#",
            theme::regular(11.5),
            theme::pal().muted,
        );
        for (i, c) in d.columns.iter().enumerate() {
            p.text(
                egui::pos2(
                    rect.left() + num_w + col_w * i as f32 + 8.0,
                    rect.center().y,
                ),
                egui::Align2::LEFT_CENTER,
                c,
                theme::regular(11.5),
                theme::pal().muted,
            );
        }
    };
    egui::ScrollArea::both()
        .id_salt("records")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            header(ui);
            for (i, row) in d.rows.iter().enumerate() {
                let r = i + 1;
                let (rect, resp) = ui
                    .allocate_exact_size(vec2(num_w + col_w * cols as f32, row_h), Sense::click());
                let label = format!("{} {r}: {}", app.i18n.t("data-record"), row.join(", "));
                resp.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Row, true, r == record, &label)
                });
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let selected = r == record;
                let p = ui.painter();
                if selected {
                    p.rect_filled(rect, CornerRadius::ZERO, theme::pal().primary_tint);
                } else if resp.hovered() {
                    p.rect_filled(rect, CornerRadius::ZERO, theme::pal().bg);
                }
                p.line_segment(
                    [rect.left_top(), rect.right_top()],
                    Stroke::new(1.0, theme::pal().border_soft),
                );
                let fg = if selected {
                    theme::pal().primary
                } else {
                    theme::pal().text
                };
                p.text(
                    rect.left_center() + vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    r.to_string(),
                    theme::regular(12.5),
                    fg,
                );
                for (c, v) in row.iter().enumerate().take(cols) {
                    let x = rect.left() + num_w + col_w * c as f32 + 8.0;
                    let clip = egui::Rect::from_min_max(
                        egui::pos2(x, rect.top()),
                        egui::pos2(x + col_w - 12.0, rect.bottom()),
                    );
                    p.with_clip_rect(clip).text(
                        egui::pos2(x, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        v,
                        theme::regular(12.5),
                        fg,
                    );
                }
                if resp.clicked() {
                    record = r;
                }
            }
        });
    app.ui.record = record;
}
