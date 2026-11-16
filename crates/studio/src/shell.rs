//! The window chrome of the redesign (design/labely-desktop.figcraft): title bar with document
//! tabs, the editor toolbar, the left icon rail and its panes, the inspector header, the canvas
//! overlays and the status bar. Every edit still goes through `LabelyApp::run`.

use egui::{Color32, CornerRadius, Margin, RichText, Sense, Stroke, Ui, vec2};
use labely_core::{Element, ElementId};
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::{LabelyApp, LeftTab};
use crate::{icons, menu, panels, theme};

/// Icon of an element kind.
pub fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "text" => "text",
        "barcode" => "barcode",
        "code2d" => "qr",
        "image" => "image",
        "rect" => "rect",
        "ellipse" => "ellipse",
        "line" => "line",
        "table" => "table",
        "icon" => "star",
        "border" => "border",
        "group" => "group",
        _ => "rect",
    }
}

/// Frame of a chrome panel (title bar, toolbar, status bar).
pub fn bar_frame(fill: Color32, x: i8, y: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(x, y))
        .stroke(Stroke::NONE)
}

/// Side panel frame with a right or left border drawn by the panel itself.
pub fn pane_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .inner_margin(Margin::symmetric(12, 12))
}

// ---------- title bar ----------

/// One tab in the title bar; returns (tab clicked, close clicked).
fn tab(
    ui: &mut Ui,
    icon: &str,
    text: &str,
    hover: &str,
    active: bool,
    close: Option<&str>,
) -> (egui::Response, bool) {
    let mut closed = false;
    let fill = if active {
        theme::SURFACE
    } else {
        Color32::TRANSPARENT
    };
    let inner = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .stroke(if active {
            Stroke::new(1.0, theme::BORDER)
        } else {
            Stroke::NONE
        })
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let color = if active { theme::PRIMARY } else { theme::TEXT2 };
            icons::show(ui, icon, 15.0, color);
            let font = if active {
                theme::bold(13.0)
            } else {
                theme::regular(13.0)
            };
            let r = ui
                .add(
                    egui::Button::new(RichText::new(text).font(font).color(theme::TEXT))
                        .frame(false),
                )
                .on_hover_text(hover);
            if let Some(tip) = close
                && ui
                    .add(
                        egui::Button::new(RichText::new("×").color(theme::MUTED).size(15.0))
                            .frame(false),
                    )
                    .on_hover_text(tip)
                    .clicked()
            {
                closed = true;
            }
            r
        });
    let r = inner.inner;
    let middle = inner.response.middle_clicked() || r.middle_clicked();
    (r, closed || middle)
}

/// Window title row: home tab, document tabs, +, then the printer and MCP pills.
pub fn title_bar(app: &mut LabelyApp, ui: &mut Ui) {
    let docs = app
        .run("doc.list", json!({}))
        .result
        .and_then(|r| r.get("docs").cloned())
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default();
    let editing = app.editing();
    let mut activate = None;
    let mut close = None;
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        let home = app.i18n.t("tab-home");
        let (r, _) = tab(ui, "home", &home, &home, !editing, None);
        if r.clicked() {
            app.ui.home = true;
        }
        egui::ScrollArea::horizontal()
            .max_width(ui.available_width() - 260.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for d in &docs {
                        let id = d["doc_id"].as_str().unwrap_or_default().to_owned();
                        let name = d["name"].as_str().unwrap_or_default();
                        let dirty = if d["dirty"].as_bool() == Some(true) {
                            " •"
                        } else {
                            ""
                        };
                        let active = editing && d["active"].as_bool() == Some(true);
                        let close_tip = app.i18n.t("cmd-doc-close");
                        let (r, closed) = tab(
                            ui,
                            "file",
                            &format!("{name}{dirty}"),
                            d["path"].as_str().unwrap_or(name),
                            active,
                            Some(&close_tip),
                        );
                        if r.clicked() {
                            activate = Some(id.clone());
                        }
                        if closed {
                            close = Some(id);
                        }
                    }
                    let new = menu::command_label(&app.i18n, "doc.new");
                    if icons::button_named(ui, "plus", "+", &new, false, 26.0).clicked() {
                        menu::invoke(app, "doc.new");
                    }
                });
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            mcp_pill(app, ui);
            device_pill(app, ui);
        });
    });
    if let Some(id) = activate {
        app.activate(&id);
        app.ui.home = false;
    }
    if let Some(id) = close {
        app.request_close(&id);
    }
}

/// A rounded pill: optional status dot or icon, then text runs. Painted directly so it lays
/// out the same in left-to-right and right-to-left rows.
fn pill(
    ui: &mut Ui,
    fill: Color32,
    dot: Option<Color32>,
    icon: Option<(&str, Color32)>,
    parts: &[(String, egui::FontId, Color32)],
) -> egui::Response {
    let galleys: Vec<_> = parts
        .iter()
        .map(|(t, f, c)| ui.painter().layout_no_wrap(t.clone(), f.clone(), *c))
        .collect();
    let lead = if dot.is_some() {
        14.0
    } else if icon.is_some() {
        21.0
    } else {
        0.0
    };
    let text_w: f32 = galleys.iter().map(|g| g.size().x).sum();
    let size = vec2(20.0 + lead + text_w, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let label: String = parts.iter().map(|p| p.0.as_str()).collect();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(13), fill);
    let mut x = rect.left() + 10.0;
    if let Some(c) = dot {
        p.circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.0, c);
    }
    if let Some((name, c)) = icon {
        icons::paint(
            p,
            egui::Rect::from_center_size(egui::pos2(x + 7.5, rect.center().y), vec2(15.0, 15.0)),
            name,
            c,
            1.5,
        );
    }
    x += lead;
    for g in galleys {
        let w = g.size().x;
        let y = rect.center().y - g.size().y / 2.0;
        p.galley(egui::pos2(x, y), g, Color32::PLACEHOLDER);
        x += w;
    }
    resp
}

/// Battery of a connected printer from its last status, percent.
pub fn battery(app: &LabelyApp, address: &str) -> Option<u64> {
    app.ui
        .device_status
        .get(address)
        .and_then(|s| s["battery_percent"].as_u64())
}

fn device_pill(app: &mut LabelyApp, ui: &mut Ui) {
    let connected = app.session.services().devices.connected();
    let r = match connected.first() {
        Some(d) => {
            let name = d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
            let transport = d.address.split(':').next().unwrap_or_default();
            let mut rest = format!(" · {}", transport.to_uppercase());
            if let Some(b) = battery(app, &d.address) {
                rest.push_str(&format!(" · {b}%"));
            }
            let summary = panels::device_summary(app, d).join(" · ");
            pill(
                ui,
                theme::GREEN_TINT,
                Some(theme::GREEN),
                None,
                &[
                    (name, theme::bold(12.0), theme::TEXT),
                    (rest, theme::regular(12.0), theme::MUTED),
                ],
            )
            .on_hover_text(summary)
        }
        None => pill(
            ui,
            theme::BG,
            Some(theme::MUTED),
            None,
            &[(
                app.i18n.t("home-printer-none"),
                theme::regular(12.0),
                theme::TEXT2,
            )],
        ),
    };
    if r.clicked() {
        if app.editing() {
            app.ui.show_printers = true;
        } else {
            app.ui.home_page = crate::home::Page::Printers;
        }
    }
}

fn mcp_pill(app: &mut LabelyApp, ui: &mut Ui) {
    let (hover, color) = match &app.mcp {
        Some(h) => (
            format!("{}\n{}", app.i18n.t("mcp-on-hint"), h.url),
            theme::PRIMARY,
        ),
        None => (app.i18n.t("mcp-off"), theme::MUTED),
    };
    pill(
        ui,
        Color32::TRANSPARENT,
        None,
        Some(("agent", color)),
        &[("MCP".to_owned(), theme::bold(12.0), color)],
    )
    .on_hover_text(hover);
}

// ---------- toolbar ----------

fn sep(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment(
        [r.center_top(), r.center_bottom()],
        Stroke::new(1.0, theme::BORDER),
    );
}

/// Editor toolbar: history, align and distribute, arrange, view toggles, preview and print.
pub fn toolbar(app: &mut LabelyApp, ui: &mut Ui) {
    let sel = app.selection();
    let n = sel.len();
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        let t = |app: &LabelyApp, id: &str| menu::command_label(&app.i18n, id);
        ui.add_enabled_ui(app.session.can_undo(None), |ui| {
            if icons::button(ui, "undo", &t(app, "history.undo"), false).clicked() {
                app.run_reporting("history.undo", json!({}));
            }
        });
        ui.add_enabled_ui(app.session.can_redo(None), |ui| {
            if icons::button(ui, "redo", &t(app, "history.redo"), false).clicked() {
                app.run_reporting("history.redo", json!({}));
            }
        });
        sep(ui);
        ui.add_enabled_ui(n >= 1, |ui| {
            for (icon, edge) in [
                ("al_left", "left"),
                ("al_center", "center"),
                ("al_right", "right"),
                ("al_top", "top"),
                ("al_middle", "middle"),
                ("al_bottom", "bottom"),
            ] {
                let tip = app.i18n.t(&format!("align-{edge}"));
                if icons::button(ui, icon, &tip, false).clicked() {
                    app.run_reporting("element.align", json!({"ids": sel, "edge": edge}));
                }
            }
        });
        sep(ui);
        ui.add_enabled_ui(n >= 3, |ui| {
            for (icon, axis) in [("dist_h", "horizontal"), ("dist_v", "vertical")] {
                let tip = app.i18n.t(&format!("distribute-{axis}"));
                if icons::button(ui, icon, &tip, false).clicked() {
                    app.run_reporting("element.distribute", json!({"ids": sel, "axis": axis}));
                }
            }
        });
        sep(ui);
        ui.add_enabled_ui(n >= 2, |ui| {
            if icons::button(ui, "group", &t(app, "element.group"), false).clicked() {
                menu::invoke(app, "element.group");
            }
        });
        ui.add_enabled_ui(n >= 1, |ui| {
            if icons::button(ui, "copy", &t(app, "element.duplicate"), false).clicked() {
                menu::invoke(app, "element.duplicate");
            }
            if icons::button(ui, "rotate", &app.i18n.t("prop-rotation-deg"), false).clicked() {
                rotate_selection(app, &sel);
            }
            if icons::button(ui, "flip", &t(app, "element.mirror"), false).clicked() {
                menu::invoke(app, "element.mirror");
            }
            if icons::button(ui, "lock", &t(app, "element.lock"), false).clicked() {
                menu::invoke(app, "element.lock");
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let print = app.i18n.t("toolbar-print");
            if icons::text_button(
                ui,
                "printer",
                &print,
                Some(theme::PRIMARY),
                Color32::WHITE,
                None,
            )
            .clicked()
            {
                menu::invoke(app, "print.submit");
            }
            let preview = app.i18n.t("panel-preview");
            if ui
                .add(
                    egui::Button::selectable(
                        app.ui.show_preview,
                        RichText::new(preview).font(theme::bold(13.0)),
                    )
                    .min_size(vec2(0.0, 32.0)),
                )
                .clicked()
            {
                app.ui.show_preview = !app.ui.show_preview;
            }
            sep(ui);
            let snap = app.i18n.t("action-snap");
            if icons::button(ui, "magnet", &snap, app.ui.snap).clicked() {
                app.ui.snap = !app.ui.snap;
            }
            let grid = app.i18n.t("action-show-grid");
            if icons::button(ui, "grid", &grid, app.ui.show_grid).clicked() {
                app.ui.show_grid = !app.ui.show_grid;
            }
        });
    });
}

/// Turn the selected elements a quarter turn clockwise.
fn rotate_selection(app: &mut LabelyApp, sel: &[ElementId]) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    for id in sel {
        if let Some(e) = doc.element(*id) {
            let deg = (e.rotation_deg + 90.0).rem_euclid(360.0);
            app.run_reporting(
                "element.set_props",
                json!({"id": id, "props": {"rotation_deg": deg}}),
            );
        }
    }
}

// ---------- left rail ----------

/// The icon rail: Add, Layers, Data, Templates; settings at the bottom.
pub fn rail(app: &mut LabelyApp, ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        for (tab, icon, key) in [
            (LeftTab::Insert, "plus", "rail-insert"),
            (LeftTab::Layers, "layers", "rail-layers"),
            (LeftTab::Data, "data", "rail-data"),
            (LeftTab::Templates, "templates", "rail-templates"),
        ] {
            let active = app.ui.left_tab == tab && app.ui.show_layers;
            if rail_item(ui, icon, &app.i18n.t(key), active).clicked() {
                if active {
                    app.ui.show_layers = false;
                } else {
                    app.ui.left_tab = tab;
                    app.ui.show_layers = true;
                }
                app.ui.show_data = app.ui.left_tab == LeftTab::Data && app.ui.show_layers;
                if tab == LeftTab::Templates {
                    app.ui.template_cache = None;
                }
            }
            ui.add_space(6.0);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            let tip = app.i18n.t("rail-settings");
            if icons::button_sized(ui, "settings", &tip, false, 32.0).clicked() {
                app.ui.home = true;
                app.ui.home_page = crate::home::Page::Settings;
            }
        });
    });
}

fn rail_item(ui: &mut Ui, icon: &str, text: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(60.0, 52.0), Sense::click());
    let enabled = ui.is_enabled();
    let label = text.to_owned();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, enabled, active, &label));
    let tile = egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 16.0), vec2(36.0, 30.0));
    let bg = if active {
        theme::PRIMARY_TINT
    } else if resp.hovered() {
        theme::BG
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(tile, CornerRadius::same(theme::RADIUS), bg);
    let color = if active { theme::PRIMARY } else { theme::TEXT2 };
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(tile.center(), vec2(19.0, 19.0)),
        icon,
        color,
        1.5,
    );
    ui.painter().text(
        rect.center_bottom() - vec2(0.0, 4.0),
        egui::Align2::CENTER_BOTTOM,
        text,
        theme::regular(11.0),
        if active { theme::PRIMARY } else { theme::MUTED },
    );
    resp
}

/// The pane next to the rail.
pub fn side_pane(app: &mut LabelyApp, ui: &mut Ui) {
    match app.ui.left_tab {
        LeftTab::Insert => insert_pane(app, ui),
        LeftTab::Layers => layers_pane(app, ui),
        LeftTab::Data => crate::data::data_pane(app, ui),
        LeftTab::Templates => crate::home::templates_pane(app, ui),
    }
}

fn pane_title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme::bold(16.0))
            .color(theme::TEXT),
    );
    ui.add_space(6.0);
}

/// A palette tile: icon over a caption; click to add, drag onto the label.
fn tile(ui: &mut Ui, icon: &str, text: &str, a11y: &str, hint: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(72.0, 62.0), Sense::click_and_drag());
    let label = a11y.to_owned();
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
    let hovered = resp.hovered();
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::RADIUS),
        if hovered {
            theme::PRIMARY_TINT
        } else {
            theme::SURFACE
        },
        Stroke::new(
            1.0,
            if hovered {
                theme::PRIMARY_TINT2
            } else {
                theme::BORDER
            },
        ),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 22.0), vec2(20.0, 20.0)),
        icon,
        theme::TEXT,
        1.5,
    );
    ui.painter().text(
        rect.center_bottom() - vec2(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        text,
        theme::regular(11.5),
        theme::TEXT2,
    );
    resp.on_hover_text(hint)
        .on_hover_cursor(egui::CursorIcon::Grab)
}

/// Insert pane: element tiles, quick bound texts, code from image, then the layer list.
fn insert_pane(app: &mut LabelyApp, ui: &mut Ui) {
    pane_title(ui, &app.i18n.t("panel-elements"));
    let hint = app.i18n.t("palette-hint");
    let mut clicked: Option<&'static str> = None;
    let mut bound: Option<&'static str> = None;
    let mut image = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (cmd, kind) in menu::PALETTE {
            let text = app.i18n.t(&format!("kind-{kind}"));
            let r = tile(ui, kind_icon(kind), &text, &format!("+ {text}"), &hint);
            if r.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), menu::ElementDrag(cmd));
            }
            if r.clicked() {
                clicked = Some(cmd);
            }
        }
        let text = app.i18n.t("kind-image");
        if tile(ui, "image", &text, &format!("+ {text}"), &hint).clicked() {
            image = true;
        }
        for (icon, key, value) in [
            ("date", "action-insert-date", "{{now:%Y-%m-%d}}"),
            ("serial", "action-insert-serial", "{{serial}}"),
        ] {
            let text = app.i18n.t(key);
            if tile(ui, icon, &text, &format!("+ {text}"), &hint).clicked() {
                bound = Some(value);
            }
        }
    });
    ui.add_space(8.0);
    let scan = app.i18n.t("action-insert-code-from-image");
    let scan_clicked = icons::text_button(
        ui,
        "scan",
        &scan,
        Some(theme::PRIMARY_TINT),
        theme::PRIMARY,
        Some(ui.available_width()),
    )
    .clicked();
    if let Some(cmd) = clicked {
        menu::invoke(app, cmd);
    }
    if image {
        menu::invoke(app, "element.add_image");
    }
    if let Some(text) = bound {
        menu::insert_bound_text(app, text);
    }
    if scan_clicked && let Some(path) = menu::pick_file(app, &["png", "jpg", "jpeg", "bmp", "webp"])
    {
        menu::insert_code_from_image(app, &json!({ "path": path }));
    }
    ui.add_space(10.0);
    ui.separator();
    let count = app.session.doc(None).map_or(0, |d| d.elements().count());
    ui.horizontal(|ui| {
        theme::section(ui, &app.i18n.t("panel-layers"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(count.to_string())
                    .size(11.0)
                    .color(theme::MUTED),
            );
        });
    });
    egui::ScrollArea::vertical()
        .id_salt("insert-layers")
        .show(ui, |ui| layer_rows(app, ui, false, ""));
}

/// Layers pane: search, the full list with lock and visibility, and arrange buttons.
fn layers_pane(app: &mut LabelyApp, ui: &mut Ui) {
    pane_title(ui, &app.i18n.t("panel-layers"));
    let mut query = std::mem::take(&mut app.ui.layer_query);
    ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text(app.i18n.t("layers-search"))
            .desired_width(f32::INFINITY),
    );
    app.ui.layer_query = query.clone();
    ui.add_space(6.0);
    let sel = app.selection();
    egui::Panel::bottom("layer-actions")
        .frame(egui::Frame::new().inner_margin(Margin::symmetric(0, 6)))
        .show(ui, |ui| {
            ui.label(
                RichText::new(app.i18n.t("layers-hint"))
                    .size(11.0)
                    .color(theme::MUTED),
            );
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_enabled_ui(sel.len() == 1, |ui| {
                    for (icon, to, key) in [
                        ("to_front", "front", "layer-front"),
                        ("up", "forward", "layer-forward"),
                        ("down", "backward", "layer-backward"),
                        ("to_back", "back", "layer-back"),
                    ] {
                        if icons::button(ui, icon, &app.i18n.t(key), false).clicked()
                            && let [id] = sel.as_slice()
                        {
                            app.run_reporting("element.reorder", json!({"id": id, "to": to}));
                        }
                    }
                });
                ui.add_enabled_ui(!sel.is_empty(), |ui| {
                    let dup = menu::command_label(&app.i18n, "element.duplicate");
                    if icons::button(ui, "copy", &dup, false).clicked() {
                        menu::invoke(app, "element.duplicate");
                    }
                    let del = menu::command_label(&app.i18n, "element.delete");
                    if icons::button(ui, "trash", &del, false).clicked() {
                        menu::invoke(app, "element.delete");
                    }
                });
            });
        });
    egui::ScrollArea::vertical()
        .id_salt("layers")
        .show(ui, |ui| layer_rows(app, ui, true, &query));
}

/// Layer rows, top of the stack first.
fn layer_rows(app: &mut LabelyApp, ui: &mut Ui, full: bool, query: &str) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let selection = app.selection();
    let q = query.to_lowercase();
    let warn: Vec<u64> = app
        .ui
        .check
        .as_ref()
        .and_then(|c| c["issues"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter(|i| i["severity"].as_str() != Some("info"))
        .filter_map(|i| i["element_id"].as_u64())
        .collect();
    for e in doc.elements().collect::<Vec<_>>().into_iter().rev() {
        let title = panels::element_title(app, e);
        if !q.is_empty() && !title.to_lowercase().contains(&q) {
            continue;
        }
        layer_row(
            app,
            ui,
            e,
            &title,
            selection.contains(&e.id),
            full,
            warn.contains(&e.id.0),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn layer_row(
    app: &mut LabelyApp,
    ui: &mut Ui,
    e: &Element,
    title: &str,
    selected: bool,
    full: bool,
    warn: bool,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
    let row = ui.interact(
        rect,
        ui.make_persistent_id(("layer", e.id.0)),
        Sense::click(),
    );
    let bg = if selected {
        theme::PRIMARY_TINT
    } else if row.hovered() {
        theme::BG
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    let fg = if selected {
        theme::PRIMARY
    } else if e.hidden {
        theme::MUTED
    } else {
        theme::TEXT
    };
    let kind = e.kind.type_name();
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.left_center() + vec2(16.0, 0.0), vec2(15.0, 15.0)),
        kind_icon(kind),
        if selected {
            theme::PRIMARY
        } else {
            theme::MUTED
        },
        1.4,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_max(
                rect.left_top() + vec2(32.0, 0.0),
                rect.right_bottom() - vec2(if full || e.locked { 60.0 } else { 34.0 }, 0.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let label = child.add(
        egui::Label::new(RichText::new(title).color(fg).font(if selected {
            theme::bold(13.0)
        } else {
            theme::regular(13.0)
        }))
        .truncate()
        .sense(Sense::click()),
    );
    let mut toggle_hidden = false;
    let mut toggle_lock = false;
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(4.0, 0.0)))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    right.spacing_mut().item_spacing.x = 0.0;
    let eye = if e.hidden { "eye_off" } else { "eye" };
    if icons::button_sized(
        &mut right,
        eye,
        &app.i18n.t("layer-toggle-visible"),
        false,
        24.0,
    )
    .clicked()
    {
        toggle_hidden = true;
    }
    if full || e.locked {
        let lock = if e.locked { "lock" } else { "unlock" };
        if icons::button_sized(
            &mut right,
            lock,
            &app.i18n.t("layer-toggle-lock"),
            false,
            24.0,
        )
        .clicked()
        {
            toggle_lock = true;
        }
    }
    if warn {
        icons::show(&mut right, "warn", 15.0, theme::AMBER);
    }
    if toggle_hidden {
        app.run_reporting(
            "element.set_props",
            json!({"id": e.id, "props": {"hidden": !e.hidden}}),
        );
    }
    if toggle_lock {
        app.run_reporting(
            "element.set_props",
            json!({"id": e.id, "props": {"locked": !e.locked}}),
        );
    }
    let clicked = row.clicked() || label.clicked();
    if clicked {
        if ui.input(|i| i.modifiers.shift || i.modifiers.command) {
            app.run("selection.set", json!({"ids": [e.id], "mode": "toggle"}));
        } else {
            app.select(&[e.id]);
        }
    }
    let menu = |ui: &mut Ui| {
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
    };
    label.union(row).context_menu(menu);
}

// ---------- inspector ----------

/// Element / Label switch at the top of the inspector, and the selected element's card.
pub fn inspector(app: &mut LabelyApp, ui: &mut Ui) {
    let selection = app.selection();
    let label_tab = selection.is_empty() || app.ui.inspector_label;
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 8.0) / 2.0;
        egui::Frame::new()
            .fill(theme::BG)
            .corner_radius(CornerRadius::same(theme::RADIUS))
            .inner_margin(Margin::same(3))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (is_label, key) in [(false, "inspector-element"), (true, "inspector-label")] {
                    let active = label_tab == is_label;
                    let text = RichText::new(app.i18n.t(key)).font(if active {
                        theme::bold(13.0)
                    } else {
                        theme::regular(13.0)
                    });
                    let r = ui.add_sized(
                        [w - 4.0, 26.0],
                        egui::Button::new(text.color(if active {
                            theme::TEXT
                        } else {
                            theme::MUTED
                        }))
                        .fill(if active {
                            theme::SURFACE
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(Stroke::NONE),
                    );
                    if r.clicked() {
                        app.ui.inspector_label = is_label;
                    }
                }
            });
    });
    ui.add_space(8.0);
    if !label_tab
        && let [id] = selection.as_slice()
        && let Some(doc) = app.session.doc_arc(None)
        && let Some(e) = doc.element(*id)
    {
        element_card(app, ui, e);
    }
    panels::properties(app, ui, label_tab);
}

fn element_card(app: &LabelyApp, ui: &mut Ui, e: &Element) {
    let kind = e.kind.type_name();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        ui.painter()
            .rect_filled(r, CornerRadius::same(theme::RADIUS), theme::PRIMARY_TINT);
        icons::paint(
            ui.painter(),
            r.shrink(7.0),
            kind_icon(kind),
            theme::PRIMARY,
            1.5,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let title = panels::element_title(app, e);
            ui.add(
                egui::Label::new(
                    RichText::new(title)
                        .font(theme::bold(14.0))
                        .color(theme::TEXT),
                )
                .truncate(),
            );
            let mut sub = app.i18n.t(&format!("kind-{kind}"));
            if let Some(col) = bound_column(e) {
                sub.push_str(&format!(" · {{{{{col}}}}}"));
            }
            ui.label(RichText::new(sub).size(11.5).color(theme::MUTED));
        });
    });
    ui.add_space(4.0);
}

/// Data column an element's text or data is bound to (`{{column}}`), if any.
fn bound_column(e: &Element) -> Option<String> {
    let v = serde_json::to_value(&e.kind).ok()?;
    let s = v
        .get("text")
        .or_else(|| v.get("data"))
        .and_then(Value::as_str)?;
    let start = s.find("{{")?;
    let end = s[start..].find("}}")? + start;
    let inner = s[start + 2..end].trim();
    (!inner.is_empty() && !inner.contains(':')).then(|| inner.to_owned())
}

// ---------- canvas overlays ----------

/// Label size pill (bottom centre) and zoom control (bottom right) over the canvas.
pub fn canvas_overlays(app: &mut LabelyApp, ui: &mut Ui, area: egui::Rect) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let units = app.units();
    let w = units.format(doc.label.width_mm);
    let size = format!(
        "{} × {}",
        w.trim_end_matches(units.symbol()).trim(),
        units.format(doc.label.height_mm)
    );
    let media = serde_json::to_value(doc.label.media)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let color_key = match app.ui.label_color {
        [255, 255, 255] => "color-white",
        [255, 236, 140] => "color-yellow",
        [255, 196, 214] => "color-pink",
        [180, 220, 255] => "color-blue",
        [196, 236, 190] => "color-green",
        _ => "color-transparent",
    };
    let media_key = format!("media-{}", media.replace('_', "-"));
    let media = if app.i18n.has(&media_key) {
        app.i18n.t(&media_key)
    } else {
        media
    };
    let paper = format!("{} · {media}", app.i18n.t(color_key));
    let float = || {
        egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(16))
            .shadow(egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(18),
            })
            .inner_margin(Margin::symmetric(10, 4))
    };
    // Centred, but clear of the zoom control on a narrow canvas.
    let cx = area.center().x.min(area.right() - 190.0 - 150.0);
    let pill_rect = egui::Rect::from_center_size(
        egui::pos2(cx.max(area.left() + 150.0), area.bottom() - 34.0),
        vec2(300.0, 32.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pill_rect).layout(
        egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Center),
    ));
    let r = float()
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            icons::show(ui, "rect", 14.0, theme::TEXT2);
            ui.label(
                RichText::new(size)
                    .font(theme::bold(12.5))
                    .color(theme::TEXT),
            );
            let [r, g, b] = app.ui.label_color;
            let (d, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle(
                d.center(),
                5.0,
                Color32::from_rgb(r, g, b),
                Stroke::new(1.0, theme::BORDER),
            );
            ui.label(RichText::new(paper).size(12.5).color(theme::TEXT2));
            icons::show(ui, "chev_down", 14.0, theme::MUTED);
        })
        .response
        .interact(Sense::click())
        .on_hover_text(app.i18n.t("canvas-paper"));
    if r.clicked() {
        menu::invoke(app, "paper.list");
    }

    let zoom_rect = egui::Rect::from_min_size(
        egui::pos2(area.right() - 170.0, area.bottom() - 52.0),
        vec2(150.0, 36.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(zoom_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    float().show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icons::button_sized(ui, "minus", &app.i18n.t("action-zoom-out"), false, 24.0).clicked() {
            app.ui.fitted = false;
            app.ui.zoom = (app.ui.zoom / 1.25).max(1.0);
        }
        let pct = format!("{:.0}%", app.ui.zoom / 8.0 * 100.0);
        if ui
            .add(
                egui::Button::new(
                    RichText::new(pct)
                        .font(theme::bold(12.5))
                        .color(theme::TEXT),
                )
                .frame(false)
                .min_size(vec2(44.0, 24.0)),
            )
            .on_hover_text(app.i18n.t("action-zoom-fit"))
            .clicked()
        {
            app.ui.fit_pending = true;
        }
        if icons::button_sized(ui, "plus", &app.i18n.t("action-zoom-in"), false, 24.0).clicked() {
            app.ui.fitted = false;
            app.ui.zoom = (app.ui.zoom * 1.25).min(80.0);
        }
        let grid = app.i18n.t("action-show-grid");
        if icons::button_sized(ui, "grid", &grid, app.ui.show_grid, 24.0).clicked() {
            app.ui.show_grid = !app.ui.show_grid;
        }
    });
}

// ---------- status bar ----------

pub fn status_bar(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let small = |t: String| RichText::new(t).size(11.5).color(theme::MUTED);
        if app.editing()
            && let Some(doc) = app.session.doc(None)
        {
            let n = doc.elements().count();
            let k = app.session.selection(None).len();
            ui.label(small(
                app.i18n
                    .t_args("status-elements", &[("count", Arg::Num(n as f64))]),
            ));
            ui.label(small("·".into()));
            ui.label(small(
                app.i18n
                    .t_args("status-selected", &[("count", Arg::Num(k as f64))]),
            ));
        }
        if let Some(n) = crate::jobs::active_count(app) {
            ui.label(small("·".into()));
            ui.label(small(
                app.i18n
                    .t_args("status-jobs", &[("count", Arg::Num(n as f64))]),
            ));
        }
        if let Some(s) = &app.ui.status
            && s.at.elapsed().as_secs() < 8
        {
            ui.label(small("·".into()));
            let color = if s.error { theme::RED } else { theme::TEXT2 };
            ui.label(RichText::new(&s.text).size(11.5).color(color));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lang = app.i18n.lang();
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(lang.code().to_uppercase())
                            .font(theme::bold(11.5))
                            .color(theme::TEXT),
                    )
                    .frame(false),
                )
                .on_hover_text(lang.native_name())
                .clicked()
            {
                let all = labely_i18n::Lang::ALL;
                let i = all.iter().position(|l| *l == lang).unwrap_or(0);
                let next = all[(i + 1) % all.len()];
                app.i18n = labely_i18n::I18n::new(next);
                app.prefs.lang = Some(next.code().to_owned());
                app.save_prefs();
            }
            match &app.mcp {
                Some(h) => {
                    ui.checkbox(
                        &mut app.ui.allow_agent_print,
                        small(app.i18n.t("action-allow-agent-print")),
                    );
                    if let Some(last) = app.ui.agent_log.back() {
                        ui.label(small(format!("⚙ {last}")));
                    }
                    let url = h.url.trim_start_matches("http://").to_owned();
                    ui.label(small(
                        app.i18n.t_args("status-agents", &[("url", Arg::Str(&url))]),
                    ))
                    .on_hover_text(app.i18n.t("mcp-on-hint"));
                    icons::show(ui, "agent", 13.0, theme::MUTED);
                }
                None => {
                    ui.label(small(app.i18n.t("mcp-off")));
                }
            }
        });
    });
}
