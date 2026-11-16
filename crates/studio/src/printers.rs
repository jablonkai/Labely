//! The printers page of the home screen (design screen 8): devices on the left, the selected
//! printer's facts and settings on the right. Every action runs a `device.*` command.

use egui::{Color32, CornerRadius, Margin, RichText, Sense, Stroke, Ui, vec2};
use labely_core::services::DeviceInfo;
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::LabelyApp;
use crate::{icons, menu, panels, theme};

/// Transport of an address, e.g. `USB` or `BLE`.
fn transport(address: &str) -> String {
    address.split(':').next().unwrap_or_default().to_uppercase()
}

fn badge(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, fg.gamma_multiply(0.25)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(fg));
        });
}

/// Printer icon on a tinted square.
fn printer_icon(ui: &mut Ui, size: f32, active: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let (bg, fg) = if active {
        (theme::PRIMARY_TINT, theme::PRIMARY)
    } else {
        (theme::BG, theme::TEXT2)
    };
    ui.painter()
        .rect_filled(r, CornerRadius::same(theme::RADIUS), bg);
    icons::paint(ui.painter(), r.shrink(size * 0.27), "printer", fg, 1.5);
}

/// A device row card; the trailing area holds a badge or a button.
fn device_row(
    ui: &mut Ui,
    name: &str,
    sub: &str,
    dot: Color32,
    selected: bool,
    trailing: impl FnOnce(&mut Ui),
) -> egui::Response {
    let stroke = if selected {
        Stroke::new(2.0, theme::PRIMARY)
    } else {
        Stroke::new(1.0, theme::BORDER)
    };
    let r = egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(theme::RADIUS_LG))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                printer_icon(ui, 44.0, selected);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new(name)
                            .font(theme::bold(14.0))
                            .color(theme::TEXT),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        let (d, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                        ui.painter().circle_filled(d.center(), 4.0, dot);
                        ui.label(RichText::new(sub).size(11.5).color(theme::MUTED));
                    });
                });
                theme::trailing(ui, trailing);
            });
        })
        .response;
    let label = name.to_owned();
    let r = r.interact(Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, &label));
    r
}

/// The selected connected printer (kept in `ui.printer`), defaulting to the first one.
fn selected(app: &mut LabelyApp, connected: &[DeviceInfo]) -> Option<DeviceInfo> {
    let keep = app
        .ui
        .printer
        .as_ref()
        .and_then(|a| connected.iter().find(|d| &d.address == a))
        .cloned();
    let pick = keep.or_else(|| connected.first().cloned());
    app.ui.printer = pick.as_ref().map(|d| d.address.clone());
    pick
}

pub fn page(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(app.i18n.t("panel-printers"))
                    .font(theme::bold(25.0))
                    .color(theme::TEXT),
            );
            ui.label(RichText::new(app.i18n.t("printers-subtitle")).color(theme::MUTED));
        });
        theme::trailing(ui, |ui| {
            if icons::text_button(
                ui,
                "search",
                &app.i18n.t("printers-search"),
                Some(theme::PRIMARY),
                Color32::WHITE,
                None,
            )
            .clicked()
            {
                menu::scan(app);
            }
        });
    });
    ui.add_space(14.0);
    let connected = app.session.services().devices.connected();
    let current = selected(app, &connected);
    let h = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        let left = (ui.available_width() * 0.4).clamp(300.0, 440.0);
        ui.allocate_ui_with_layout(
            vec2(left, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(left);
                egui::ScrollArea::vertical()
                    .id_salt("printer-list")
                    .show(ui, |ui| devices(app, ui, &connected));
            },
        );
        let right = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(right, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                theme::card().inner_margin(Margin::same(20)).show(ui, |ui| {
                    ui.set_width(right - 42.0);
                    ui.set_min_height(h - 42.0);
                    egui::ScrollArea::vertical()
                        .id_salt("printer-detail")
                        .show(ui, |ui| match &current {
                            Some(d) => details(app, ui, d),
                            None => {
                                ui.add_space(30.0);
                                ui.vertical_centered(|ui| {
                                    printer_icon(ui, 56.0, false);
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(app.i18n.t("printers-none"))
                                            .color(theme::MUTED),
                                    );
                                });
                                ui.add_space(20.0);
                                crate::jobs::queue_section(app, ui);
                            }
                        });
                });
            },
        );
    });
}

fn devices(app: &mut LabelyApp, ui: &mut Ui, connected: &[DeviceInfo]) {
    theme::section(ui, &app.i18n.t("printers-mine"));
    if connected.is_empty() {
        ui.label(RichText::new(app.i18n.t("printers-none-connected")).color(theme::MUTED));
    }
    for d in connected {
        let name = d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
        let mut sub = vec![transport(&d.address)];
        sub.extend(panels::device_summary(app, d));
        let active = app.ui.printer.as_deref() == Some(d.address.as_str());
        let text = app.i18n.t("printers-connected");
        if device_row(ui, &name, &sub.join(" · "), theme::GREEN, active, |ui| {
            badge(ui, &text, theme::GREEN, theme::GREEN_TINT);
        })
        .clicked()
        {
            app.ui.printer = Some(d.address.clone());
        }
        ui.add_space(4.0);
    }
    if let Some(address) = app.ui.connect_failed.clone() {
        panels::connect_help(app, ui, &address);
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        theme::section(ui, &app.i18n.t("printers-nearby"));
        theme::trailing(ui, |ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(app.i18n.t("printers-rescan"))
                            .size(12.0)
                            .color(theme::PRIMARY),
                    )
                    .frame(false),
                )
                .clicked()
            {
                menu::scan(app);
            }
        });
    });
    let mut connect = None;
    for d in app.ui.scanned.clone() {
        let address = d["address"].as_str().unwrap_or("").to_owned();
        if connected.iter().any(|c| c.address == address) {
            continue;
        }
        let name = d["name"].as_str().unwrap_or("?").to_owned();
        let sub = format!("{} · {address}", transport(&address));
        let label = app.i18n.t("action-connect");
        device_row(ui, &name, &sub, theme::PRIMARY, false, |ui| {
            if ui
                .add(egui::Button::new(label).min_size(vec2(0.0, 30.0)))
                .clicked()
            {
                connect = Some(address.clone());
            }
        });
        ui.add_space(4.0);
    }
    if let Some(a) = connect
        && menu::connect(app, &a)
    {
        app.ui.printer = Some(a.clone());
        crate::paper::detect(app, Some(&a));
    }
    ui.add_space(6.0);
    // Manual address: a dashed row with a field and Connect.
    egui::Frame::new()
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(theme::RADIUS_LG))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, "plus", 15.0, theme::TEXT2);
                let w = ui.available_width() - 100.0;
                let l = ui
                    .add(
                        egui::TextEdit::singleline(&mut app.ui.manual_address)
                            .hint_text(app.i18n.t("printers-manual"))
                            .desired_width(w),
                    )
                    .on_hover_text(app.i18n.t("printers-manual-hint"));
                let ok = !app.ui.manual_address.trim().is_empty();
                if ui
                    .add_enabled(ok, egui::Button::new(app.i18n.t("action-connect")))
                    .clicked()
                    || (ok && l.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    let a = app.ui.manual_address.trim().to_owned();
                    if menu::connect(app, &a) {
                        app.ui.printer = Some(a);
                        app.ui.manual_address.clear();
                    }
                }
            });
        });
    ui.add_space(14.0);
    ui.horizontal_top(|ui| {
        icons::show(ui, "info", 14.0, theme::MUTED);
        ui.add(
            egui::Label::new(
                RichText::new(app.i18n.t("printers-tip"))
                    .size(11.5)
                    .color(theme::MUTED),
            )
            .wrap(),
        );
    });
}

/// A small stat tile: caption, big value, note, optional bar.
fn tile(
    ui: &mut Ui,
    width: f32,
    icon: &str,
    caption: &str,
    value: &str,
    note: &str,
    bar: Option<f32>,
) {
    egui::Frame::new()
        .fill(theme::BG)
        .corner_radius(CornerRadius::same(theme::RADIUS_LG))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                icons::show(ui, icon, 13.0, theme::MUTED);
                ui.label(RichText::new(caption).size(11.5).color(theme::MUTED));
            });
            ui.label(
                RichText::new(value)
                    .font(theme::bold(19.0))
                    .color(theme::TEXT),
            );
            ui.label(RichText::new(note).size(11.5).color(theme::MUTED));
            if let Some(f) = bar {
                let (r, _) = ui.allocate_exact_size(vec2(width - 44.0, 4.0), Sense::hover());
                ui.painter()
                    .rect_filled(r, CornerRadius::same(2), theme::BORDER);
                let mut fill = r;
                fill.set_width(r.width() * f.clamp(0.0, 1.0));
                ui.painter()
                    .rect_filled(fill, CornerRadius::same(2), theme::GREEN);
            }
        });
}

/// Title, actions, stat tiles and settings rows of a connected printer.
fn details(app: &mut LabelyApp, ui: &mut Ui, d: &DeviceInfo) {
    let name = d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
    ui.horizontal(|ui| {
        printer_icon(ui, 84.0, true);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&name)
                        .font(theme::bold(21.0))
                        .color(theme::TEXT),
                );
                badge(
                    ui,
                    &format!(
                        "{} · {}",
                        app.i18n.t("printers-connected"),
                        transport(&d.address)
                    ),
                    theme::GREEN,
                    theme::GREEN_TINT,
                );
            });
            let mut facts = Vec::new();
            if let Some(sn) = &d.serial {
                facts.push(
                    app.i18n
                        .t_args("device-serial", &[("serial", Arg::Str(sn))]),
                );
            }
            if let Some(fw) = &d.firmware {
                facts.push(
                    app.i18n
                        .t_args("device-firmware", &[("version", Arg::Str(fw))]),
                );
            }
            if facts.is_empty() {
                facts.push(d.address.clone());
            }
            ui.label(
                RichText::new(facts.join(" · "))
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if icons::text_button(
                    ui,
                    "rotate",
                    &app.i18n.t("action-status"),
                    None,
                    theme::TEXT,
                    None,
                )
                .clicked()
                {
                    menu::refresh_status(app, &d.address);
                }
                if ui
                    .add(
                        egui::Button::new(app.i18n.t("device-calibrate-button"))
                            .min_size(vec2(0.0, 30.0)),
                    )
                    .clicked()
                {
                    crate::paper::open_device(app, Some(d.address.clone()));
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(app.i18n.t("action-disconnect")).color(theme::PRIMARY),
                        )
                        .frame(false),
                    )
                    .clicked()
                    && app
                        .run_reporting("device.disconnect", json!({"address": d.address}))
                        .ok
                {
                    if app.ui.detected.as_ref().and_then(|x| x["address"].as_str())
                        == Some(d.address.as_str())
                    {
                        app.ui.detected = None;
                    }
                    app.ui.printer = None;
                }
            });
        });
    });
    ui.add_space(16.0);
    let status = app
        .ui
        .device_status
        .get(&d.address)
        .cloned()
        .unwrap_or(Value::Null);
    let gap = 10.0;
    let w = (ui.available_width() - gap * 3.0) / 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        let battery = status["battery_percent"].as_u64();
        tile(
            ui,
            w,
            "battery",
            &app.i18n.t("printers-battery"),
            &battery.map_or("–".into(), |b| format!("{b}%")),
            &app.i18n.t("printers-battery-note"),
            battery.map(|b| b as f32 / 100.0),
        );
        let paper = app
            .ui
            .detected
            .as_ref()
            .filter(|x| x["address"].as_str() == Some(d.address.as_str()))
            .and_then(|x| x.get("paper").cloned());
        let (pv, pn) = match &paper {
            Some(p) => (
                match (p["width_mm"].as_f64(), p["height_mm"].as_f64()) {
                    (Some(w), Some(h)) => {
                        format!("{} × {} mm", crate::home::mm(w), crate::home::mm(h))
                    }
                    _ => "–".into(),
                },
                p["name"].as_str().unwrap_or("").to_owned(),
            ),
            None => ("–".into(), app.i18n.t("printers-paper-unknown")),
        };
        tile(ui, w, "rect", &app.i18n.t("printers-paper"), &pv, &pn, None);
        let (dv, dn) = match &d.model {
            Some(m) => (
                format!("{} dpi", m.dpi),
                app.i18n.t_args(
                    "printers-width",
                    &[("mm", Arg::Num((m.max_width_mm * 10.0).round() / 10.0))],
                ),
            ),
            None => ("–".into(), String::new()),
        };
        tile(
            ui,
            w,
            "grid",
            &app.i18n.t("printers-resolution"),
            &dv,
            &dn,
            None,
        );
        let paper_count = &status["paper"];
        let (lv, ln) = match (paper_count["total"].as_u64(), paper_count["used"].as_u64()) {
            (Some(t), Some(u)) => (
                t.saturating_sub(u).to_string(),
                app.i18n
                    .t_args("printers-labels-of", &[("total", Arg::Num(t as f64))]),
            ),
            _ => ("–".into(), app.i18n.t("printers-labels-unknown")),
        };
        tile(
            ui,
            w,
            "clock",
            &app.i18n.t("printers-labels"),
            &lv,
            &ln,
            None,
        );
    });
    crate::paper::banner(app, ui);
    ui.add_space(14.0);
    let row = |ui: &mut Ui, title: &str, sub: &str, add: &mut dyn FnMut(&mut Ui)| {
        ui.separator();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new(title)
                        .font(theme::bold(13.0))
                        .color(theme::TEXT),
                );
                if !sub.is_empty() {
                    ui.label(RichText::new(sub).size(11.5).color(theme::MUTED));
                }
            });
            theme::trailing(ui, |ui| add(ui));
        });
    };
    let mut allow = app.ui.allow_agent_print;
    row(
        ui,
        &app.i18n.t("printers-agents"),
        &app.i18n.t("printers-agents-hint"),
        &mut |ui| {
            crate::inspector::switch(ui, &mut allow)
                .on_hover_text(app.i18n.t("action-allow-agent-print"));
        },
    );
    app.ui.allow_agent_print = allow;
    let settings = app.i18n.t("action-printer-settings");
    let mut open = false;
    row(
        ui,
        &app.i18n.t("device-title"),
        &app.i18n.t("printers-settings-hint"),
        &mut |ui| {
            open = ui
                .add(egui::Button::new(&settings).min_size(vec2(0.0, 30.0)))
                .clicked();
        },
    );
    if open {
        crate::paper::open_device(app, Some(d.address.clone()));
    }
    ui.separator();
    ui.add_space(8.0);
    crate::jobs::queue_section(app, ui);
}
