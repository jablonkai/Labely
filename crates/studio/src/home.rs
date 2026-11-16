//! Home screen (design screen 1): navigation, quick actions, quick sizes, templates, recent
//! files and the printer card. Also the templates pane of the editor's left rail.

use egui::{Color32, CornerRadius, Margin, RichText, Sense, Stroke, Ui, vec2};
use labely_core::commands::template::TemplateInfo;
use serde_json::json;

use crate::app::{Dialog, LabelyApp};
use crate::dialogs::NewDocForm;
use crate::{icons, menu, templates, theme};

/// Which page the home screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    Printers,
    Settings,
    History,
    Templates,
}

/// Quick sizes, mm.
pub(crate) const QUICK_SIZES: [(f64, f64); 7] = [
    (50.0, 30.0),
    (40.0, 30.0),
    (30.0, 15.0),
    (50.0, 50.0),
    (40.0, 12.0),
    (70.0, 40.0),
    (14.0, 30.0),
];

/// Template listing, read once and kept until the library may have changed.
fn cached_templates(app: &mut LabelyApp) -> Vec<TemplateInfo> {
    if app.ui.template_cache.is_none() {
        app.ui.template_cache = Some(templates::list(app).map(|l| l.0).unwrap_or_default());
    }
    app.ui.template_cache.clone().unwrap_or_default()
}

pub(crate) fn mm(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The whole home screen.
pub fn home(app: &mut LabelyApp, ui: &mut Ui) {
    egui::Panel::left("home-nav")
        .exact_size(236.0)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(theme::CHROME)
                .inner_margin(Margin::same(14)),
        )
        .show(ui, |ui| nav(app, ui));
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(Margin::symmetric(30, 22)),
        )
        .show(ui, |ui| match app.ui.home_page {
            Page::Home => main(app, ui),
            Page::Printers => crate::printers::page(app, ui),
            Page::Settings => settings(app, ui),
            Page::History => crate::jobs::history_page(app, ui),
            Page::Templates => templates::library_page(app, ui),
        });
}

fn nav_item(ui: &mut Ui, icon: &str, text: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let label = text.to_owned();
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, enabled, active, &label));
    let bg = if active {
        theme::PRIMARY_TINT
    } else if resp.hovered() {
        theme::BG
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(theme::RADIUS), bg);
    let color = if active { theme::PRIMARY } else { theme::TEXT2 };
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(18.0, 18.0)),
        icon,
        color,
        1.5,
    );
    theme::fit_text(
        ui.painter(),
        rect.left_center() + vec2(40.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        if active {
            theme::bold(13.5)
        } else {
            theme::regular(13.5)
        },
        if active {
            theme::PRIMARY_DARK
        } else {
            theme::TEXT
        },
        rect.right() - rect.left() - 48.0,
    );
    resp
}

fn nav(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        ui.painter()
            .rect_filled(r, CornerRadius::same(7), theme::PRIMARY);
        icons::paint(
            ui.painter(),
            r.shrink(6.0),
            "templates",
            Color32::WHITE,
            1.6,
        );
        ui.label(
            RichText::new("Labely")
                .font(theme::bold(19.0))
                .color(theme::TEXT),
        );
    });
    ui.add_space(16.0);
    let page = app.ui.home_page;
    if nav_item(ui, "home", &app.i18n.t("tab-home"), page == Page::Home).clicked() {
        app.ui.home_page = Page::Home;
    }
    if nav_item(
        ui,
        "templates",
        &app.i18n.t("rail-templates"),
        page == Page::Templates,
    )
    .clicked()
    {
        menu::invoke(app, "template.new");
    }
    if nav_item(
        ui,
        "clock",
        &app.i18n.t("home-history"),
        page == Page::History,
    )
    .clicked()
    {
        menu::invoke(app, "job.history");
    }
    if nav_item(
        ui,
        "printer",
        &app.i18n.t("panel-printers"),
        page == Page::Printers,
    )
    .clicked()
    {
        app.ui.home_page = Page::Printers;
    }
    if nav_item(
        ui,
        "settings",
        &app.i18n.t("home-settings"),
        page == Page::Settings,
    )
    .clicked()
    {
        app.ui.home_page = Page::Settings;
    }
    at_bottom(ui, "home-device-card", |ui| device_card(app, ui));
}

/// Place content at the bottom of the remaining space, sized to its content (its height is
/// measured on the previous frame).
fn at_bottom(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(key);
    let last = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let space = ui.available_height() - last - ui.spacing().item_spacing.y;
    if space > 0.0 {
        ui.add_space(space);
    }
    let h = ui.scope(add).response.rect.height();
    if (h - last).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, h));
        ui.ctx().request_repaint();
    }
}

/// The connected printer: name, state, battery and loaded paper.
fn device_card(app: &mut LabelyApp, ui: &mut Ui) {
    let connected = app.session.services().devices.connected();
    let resp = theme::card()
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                let Some(d) = connected.first() else {
                    ui.horizontal(|ui| {
                        icons::show(ui, "printer", 18.0, theme::MUTED);
                        ui.add(
                            egui::Label::new(
                                RichText::new(app.i18n.t("home-printer-none")).color(theme::TEXT2),
                            )
                            .wrap(),
                        );
                    });
                    return;
                };
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                    ui.painter().rect_filled(
                        r,
                        CornerRadius::same(theme::RADIUS),
                        theme::GREEN_TINT,
                    );
                    icons::paint(ui.painter(), r.shrink(7.0), "printer", theme::GREEN, 1.5);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let name = d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
                        ui.label(
                            RichText::new(name)
                                .font(theme::bold(13.5))
                                .color(theme::TEXT),
                        );
                        let transport = d
                            .address
                            .split(':')
                            .next()
                            .unwrap_or_default()
                            .to_uppercase();
                        ui.label(
                            RichText::new(format!(
                                "{} · {transport}",
                                app.i18n.t("home-printer-connected")
                            ))
                            .size(11.5)
                            .color(theme::GREEN),
                        );
                    });
                });
                if let Some(b) = crate::shell::battery(app, &d.address) {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t("home-battery"))
                                .size(11.5)
                                .color(theme::MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{b}%")).font(theme::bold(11.5)));
                        });
                    });
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(3), theme::BORDER_SOFT);
                    let mut fill = r;
                    fill.set_width(r.width() * (b.min(100) as f32 / 100.0));
                    ui.painter()
                        .rect_filled(fill, CornerRadius::same(3), theme::GREEN);
                }
                if let Some(doc) = app.session.doc(None) {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t("home-paper"))
                                .size(11.5)
                                .color(theme::MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} × {} mm",
                                    mm(doc.label.width_mm),
                                    mm(doc.label.height_mm)
                                ))
                                .size(11.5),
                            );
                        });
                    });
                }
            });
        })
        .response
        .interact(Sense::click());
    if resp.clicked() {
        app.ui.home_page = Page::Printers;
    }
}

/// A quick-action card; `primary` is the filled blue one.
fn action_card(
    ui: &mut Ui,
    width: f32,
    icon: &str,
    title: &str,
    hint: &str,
    a11y: &str,
    primary: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 82.0), Sense::click());
    let label = a11y.to_owned();
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
    let hovered = resp.hovered();
    let (fill, stroke, fg, sub, tile, tile_fg) = if primary {
        (
            if hovered {
                theme::PRIMARY_DARK
            } else {
                theme::PRIMARY
            },
            Stroke::NONE,
            Color32::WHITE,
            Color32::from_white_alpha(210),
            Color32::from_white_alpha(40),
            Color32::WHITE,
        )
    } else {
        (
            theme::SURFACE,
            Stroke::new(
                1.0,
                if hovered {
                    theme::PRIMARY_TINT2
                } else {
                    theme::BORDER
                },
            ),
            theme::TEXT,
            theme::MUTED,
            theme::PRIMARY_TINT,
            theme::PRIMARY,
        )
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(theme::RADIUS_LG),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    let t = egui::Rect::from_center_size(rect.left_center() + vec2(38.0, 0.0), vec2(40.0, 40.0));
    p.rect_filled(t, CornerRadius::same(theme::RADIUS), tile);
    icons::paint(p, t.shrink(10.0), icon, tile_fg, 1.6);
    let x = rect.left() + 72.0;
    p.text(
        egui::pos2(x, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        title,
        theme::bold(14.5),
        fg,
    );
    let galley = p.layout(
        hint.to_owned(),
        theme::regular(12.0),
        sub,
        rect.right() - x - 12.0,
    );
    p.galley(egui::pos2(x, rect.top() + 40.0), galley, sub);
    resp
}

pub(crate) fn chip(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(if active {
            theme::PRIMARY_DARK
        } else {
            theme::TEXT
        }))
        .fill(if active {
            theme::PRIMARY_TINT
        } else {
            theme::SURFACE
        })
        .stroke(Stroke::new(
            1.0,
            if active {
                theme::PRIMARY_TINT2
            } else {
                theme::BORDER
            },
        ))
        .corner_radius(CornerRadius::same(14))
        .min_size(vec2(0.0, 28.0)),
    )
}

fn main(app: &mut LabelyApp, ui: &mut Ui) {
    // Header: greeting, title, search.
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(app.i18n.t("home-greeting"))
                    .size(13.0)
                    .color(theme::MUTED),
            );
            ui.label(
                RichText::new(app.i18n.t("home-title"))
                    .font(theme::bold(25.0))
                    .color(theme::TEXT),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut q = std::mem::take(&mut app.ui.home_query);
            ui.add(
                egui::TextEdit::singleline(&mut q)
                    .hint_text(app.i18n.t("home-search"))
                    .desired_width(300.0)
                    .margin(Margin::symmetric(10, 8)),
            );
            app.ui.home_query = q;
        });
    });
    ui.add_space(14.0);

    // Quick actions.
    let gap = 12.0;
    let w = ((ui.available_width() - 3.0 * gap) / 4.0).max(150.0);
    let new_label = menu::command_label(&app.i18n, "doc.new");
    let from_template = menu::command_label(&app.i18n, "template.new");
    let open = menu::command_label(&app.i18n, "doc.open");
    let data = menu::command_label(&app.i18n, "data.attach_csv");
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (icon, title, hint, a11y, primary, id) in [
            (
                "plus",
                "home-new-title",
                "home-new-hint",
                &new_label,
                true,
                "doc.new",
            ),
            (
                "templates",
                "home-template-title",
                "home-template-hint",
                &from_template,
                false,
                "template.new",
            ),
            (
                "data",
                "home-data-title",
                "home-data-hint",
                &data,
                false,
                "data",
            ),
            (
                "folder",
                "home-open-title",
                "home-open-hint",
                &open,
                false,
                "doc.open",
            ),
        ] {
            let r = action_card(
                ui,
                w,
                icon,
                &app.i18n.t(title),
                &app.i18n.t(hint),
                a11y,
                primary,
            );
            if r.clicked() {
                action = Some(id);
            }
        }
    });
    match action {
        // Labels from data: a new label, then pick the CSV for it.
        Some("data") => {
            let f = NewDocForm::default();
            if app
                .run_reporting(
                    "doc.new",
                    json!({"width_mm": f.width_mm, "height_mm": f.height_mm, "printer_model": f.printer_model}),
                )
                .ok
            {
                app.ui.fit_pending = true;
                app.ui.left_tab = crate::app::LeftTab::Data;
                app.ui.show_data = true;
                app.ui.show_layers = true;
                menu::invoke(app, "data.attach_csv");
            }
        }
        Some(id) => menu::invoke(app, id),
        None => {}
    }
    ui.add_space(12.0);

    // Quick sizes.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(app.i18n.t("home-quick-size"))
                .size(12.5)
                .color(theme::MUTED),
        );
        for (i, (w, h)) in QUICK_SIZES.iter().enumerate() {
            if chip(ui, &format!("{} × {} mm", mm(*w), mm(*h)), i == 0).clicked() {
                app.ui.dialog = Some(Dialog::NewDoc(NewDocForm {
                    width_mm: *w,
                    height_mm: *h,
                    ..NewDocForm::default()
                }));
            }
        }
        if chip(ui, &format!("+ {}", app.i18n.t("home-custom-size")), false).clicked() {
            menu::invoke(app, "doc.new");
        }
    });
    ui.add_space(16.0);

    // Templates (left) and recent files (right).
    let right_w = 320.0_f32.min(ui.available_width() * 0.32);
    egui::Panel::right("home-recent")
        .exact_size(right_w + 16.0)
        .resizable(false)
        .frame(egui::Frame::new().inner_margin(Margin {
            left: 16,
            ..Default::default()
        }))
        .show(ui, |ui| recent(app, ui));
    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("home-templates")
                .auto_shrink([false, false])
                .show(ui, |ui| template_sections(app, ui));
        });
}

fn template_sections(app: &mut LabelyApp, ui: &mut Ui) {
    let q = app.ui.home_query.to_lowercase();
    let all = cached_templates(app);
    let matches = |app: &LabelyApp, t: &TemplateInfo| {
        q.is_empty() || templates::display_name(app, t).to_lowercase().contains(&q)
    };
    let starters: Vec<_> = all
        .iter()
        .filter(|t| t.starter && matches(app, t))
        .cloned()
        .collect();
    let own: Vec<_> = all
        .iter()
        .filter(|t| !t.starter && matches(app, t))
        .cloned()
        .collect();
    let mut open = None;
    for (key, list) in [("home-starters", starters), ("home-own", own)] {
        if list.is_empty() {
            continue;
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(app.i18n.t(key))
                    .font(theme::bold(15.5))
                    .color(theme::TEXT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(app.i18n.t("home-all"))
                                .size(12.5)
                                .color(theme::PRIMARY),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    menu::invoke(app, "template.new");
                }
            });
        });
        ui.add_space(6.0);
        let gap = 12.0;
        let cols = ((ui.available_width() + gap) / (170.0 + gap))
            .floor()
            .max(1.0);
        let w = (ui.available_width() - gap * (cols - 1.0)) / cols;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            for t in &list {
                if template_card(app, ui, t, w).clicked() {
                    open = Some(t.reference.clone());
                }
            }
        });
        ui.add_space(18.0);
    }
    if let Some(r) = open
        && templates::open_template(app, &r)
    {
        app.ui.home = false;
    }
}

/// Template card: thumbnail on a grey well, name and size.
fn template_card(app: &mut LabelyApp, ui: &mut Ui, t: &TemplateInfo, width: f32) -> egui::Response {
    let height = width * 0.62 + 50.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let name = templates::display_name(app, t);
    let size = format!("{} × {} mm", mm(t.width_mm), mm(t.height_mm));
    let label = format!("{name} · {size}");
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
    let hovered = resp.hovered();
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::RADIUS_LG),
        theme::SURFACE,
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
    let well = egui::Rect::from_min_size(
        rect.min + vec2(8.0, 8.0),
        vec2(width - 16.0, width * 0.62 - 8.0),
    );
    ui.painter()
        .rect_filled(well, CornerRadius::same(theme::RADIUS), theme::CANVAS);
    let ctx = ui.ctx().clone();
    if let Some(tex) = templates::thumbnail(app, &ctx, t) {
        let s = tex.size_vec2();
        let k = ((well.width() - 20.0) / s.x)
            .min((well.height() - 16.0) / s.y)
            .min(1.5);
        let r = egui::Rect::from_center_size(well.center(), s * k);
        ui.painter().rect_filled(
            r.translate(vec2(0.0, 1.0)),
            CornerRadius::same(4),
            Color32::from_black_alpha(18),
        );
        egui::Image::new((tex.id(), s * k))
            .corner_radius(CornerRadius::same(4))
            .paint_at(ui, r);
    }
    theme::fit_text(
        ui.painter(),
        egui::pos2(rect.left() + 12.0, well.bottom() + 10.0),
        egui::Align2::LEFT_TOP,
        &name,
        theme::bold(13.5),
        theme::TEXT,
        rect.width() - 24.0,
    );
    theme::fit_text(
        ui.painter(),
        egui::pos2(rect.left() + 12.0, well.bottom() + 28.0),
        egui::Align2::LEFT_TOP,
        &size,
        theme::regular(11.5),
        theme::MUTED,
        rect.width() - 24.0,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Recent files card and the agents note.
fn recent(app: &mut LabelyApp, ui: &mut Ui) {
    let q = app.ui.home_query.to_lowercase();
    let mut open = None;
    let full = ui.available_height();
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(full - 30.0);
        ui.label(
            RichText::new(app.i18n.t("menu-recent"))
                .font(theme::bold(15.5))
                .color(theme::TEXT),
        );
        ui.add_space(8.0);
        let recent = app.prefs.recent.clone();
        if recent.is_empty() {
            ui.label(
                RichText::new(app.i18n.t("home-recent-empty"))
                    .size(12.5)
                    .color(theme::MUTED),
            );
        }
        for path in &recent {
            let file = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            if !q.is_empty() && !file.to_lowercase().contains(&q) {
                continue;
            }
            let stem = path
                .file_stem()
                .map_or_else(|| file.clone(), |s| s.to_string_lossy().into_owned());
            let folder = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
            let label = file.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &label));
            if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(theme::RADIUS), theme::BG);
            }
            let t = egui::Rect::from_center_size(
                rect.left_center() + vec2(22.0, 0.0),
                vec2(32.0, 32.0),
            );
            ui.painter()
                .rect_filled(t, CornerRadius::same(theme::RADIUS), theme::PRIMARY_TINT);
            icons::paint(ui.painter(), t.shrink(8.0), "file", theme::PRIMARY, 1.5);
            theme::fit_text(
                ui.painter(),
                egui::pos2(rect.left() + 48.0, rect.center().y - 9.0),
                egui::Align2::LEFT_CENTER,
                &stem,
                theme::bold(13.5),
                theme::TEXT,
                rect.width() - 56.0,
            );
            theme::fit_text(
                ui.painter(),
                egui::pos2(rect.left() + 48.0, rect.center().y + 9.0),
                egui::Align2::LEFT_CENTER,
                &folder,
                theme::regular(11.5),
                theme::MUTED,
                rect.width() - 56.0,
            );
            let resp = resp.on_hover_text(path.display().to_string());
            if resp.clicked() {
                open = Some(path.clone());
            }
        }
        at_bottom(ui, "home-agents-card", |ui| {
            egui::Frame::new()
                .fill(theme::PRIMARY_TINT)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            icons::show(ui, "agent", 16.0, theme::PRIMARY);
                            ui.label(
                                RichText::new(app.i18n.t("home-agents-title"))
                                    .font(theme::bold(13.0))
                                    .color(theme::TEXT),
                            );
                        });
                        ui.label(
                            RichText::new(app.i18n.t("home-agents-text"))
                                .size(12.5)
                                .color(theme::TEXT2),
                        );
                    });
                });
        });
    });
    if let Some(path) = open {
        app.open_recent(&path);
    }
}

/// Settings page: language, units, view and agents.
/// Settings sections: key, icon, title key.
const SETTINGS: [(&str, &str, &str); 5] = [
    ("general", "settings", "settings-general"),
    ("appearance", "sparkle", "settings-appearance"),
    ("editor", "pencil", "settings-editor"),
    ("agents", "agent", "settings-agents"),
    ("about", "info", "settings-about"),
];

/// A settings row: bold title with an optional hint on the left, the control on the right.
fn setting_row(ui: &mut Ui, title: &str, hint: &str, first: bool, add: impl FnOnce(&mut Ui)) {
    if !first {
        ui.add_space(4.0);
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .rect_filled(r, CornerRadius::ZERO, theme::BORDER_SOFT);
        ui.add_space(4.0);
    }
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() * 0.55);
            ui.spacing_mut().item_spacing.y = 1.0;
            if hint.is_empty() {
                // One line: centre it on the control's height.
                ui.add_space(6.0);
            }
            ui.label(
                RichText::new(title)
                    .font(theme::bold(13.0))
                    .color(theme::TEXT),
            );
            if !hint.is_empty() {
                ui.add(egui::Label::new(RichText::new(hint).size(11.5).color(theme::MUTED)).wrap());
            }
        });
        theme::trailing(ui, add);
    });
}

/// A settings card with an icon title; scrolled into view when its nav item is picked.
fn settings_card(
    app: &mut LabelyApp,
    ui: &mut Ui,
    key: &str,
    add: impl FnOnce(&mut LabelyApp, &mut Ui),
) {
    let (_, icon, title) = SETTINGS
        .iter()
        .find(|s| s.0 == key)
        .copied()
        .unwrap_or(SETTINGS[0]);
    let r = theme::card()
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, icon, 16.0, theme::PRIMARY);
                ui.label(
                    RichText::new(app.i18n.t(title))
                        .font(theme::bold(15.0))
                        .color(theme::TEXT),
                );
            });
            ui.add_space(6.0);
            add(app, ui);
        })
        .response;
    if app.ui.settings_jump.as_deref() == Some(key) {
        ui.scroll_to_rect(r.rect, Some(egui::Align::TOP));
        app.ui.settings_jump = None;
    }
    ui.add_space(14.0);
}

fn settings(app: &mut LabelyApp, ui: &mut Ui) {
    ui.label(
        RichText::new(app.i18n.t("home-settings"))
            .font(theme::bold(25.0))
            .color(theme::TEXT),
    );
    ui.label(RichText::new(app.i18n.t("settings-subtitle")).color(theme::MUTED));
    ui.add_space(14.0);
    let h = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        ui.allocate_ui_with_layout(
            vec2(190.0, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                theme::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_width(174.0);
                    for (key, icon, title) in SETTINGS {
                        let active = app.ui.settings_section == key;
                        if nav_item(ui, icon, &app.i18n.t(title), active).clicked() {
                            app.ui.settings_section = key.to_owned();
                            app.ui.settings_jump = Some(key.to_owned());
                        }
                    }
                });
            },
        );
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("settings")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(640.0);
                    settings_cards(app, ui);
                });
        });
    });
}

fn settings_cards(app: &mut LabelyApp, ui: &mut Ui) {
    settings_card(app, ui, "general", |app, ui| {
        let (lang_t, lang_h) = (
            app.i18n.t("home-language"),
            app.i18n.t("settings-language-hint"),
        );
        setting_row(ui, &lang_t, &lang_h, true, |ui| {
            let cur = app.i18n.lang();
            let mut pick = cur;
            egui::ComboBox::from_id_salt("settings-lang")
                .selected_text(cur.native_name())
                .width(170.0)
                .show_ui(ui, |ui| {
                    for l in labely_i18n::Lang::ALL {
                        ui.selectable_value(&mut pick, l, l.native_name());
                    }
                })
                .response
                .on_hover_text(lang_t.clone());
            if pick != cur {
                app.i18n = labely_i18n::I18n::new(pick);
                app.prefs.lang = Some(pick.code().to_owned());
                app.save_prefs();
            }
        });
        let units_t = app.i18n.t("menu-units");
        setting_row(ui, &units_t, "", false, |ui| {
            let all = crate::prefs::Units::ALL;
            let items: Vec<_> = all.iter().map(|u| (app.i18n.t(u.key()), None)).collect();
            let sel = all.iter().position(|u| *u == app.prefs.units).unwrap_or(0);
            // Left to right inside the right-aligned row, so the items keep their order.
            let picked = ui
                .allocate_ui_with_layout(
                    vec2(240.0, 34.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| crate::inspector::segmented(ui, &items, sel),
                )
                .inner;
            if let Some(i) = picked {
                app.prefs.units = all[i];
                app.save_prefs();
            }
        });
    });
    settings_card(app, ui, "appearance", |app, ui| {
        let t = app.i18n.t("action-show-rulers");
        setting_row(ui, &t, "", true, |ui| {
            crate::inspector::switch(ui, &mut app.ui.show_rulers).on_hover_text(t.clone());
        });
        let t = app.i18n.t("action-show-grid");
        setting_row(ui, &t, "", false, |ui| {
            crate::inspector::switch(ui, &mut app.ui.show_grid).on_hover_text(t.clone());
        });
    });
    settings_card(app, ui, "editor", |app, ui| {
        let (t, hint) = (
            app.i18n.t("settings-grid"),
            app.i18n.t("settings-grid-hint"),
        );
        let units = app.units();
        setting_row(ui, &t, &hint, true, |ui| {
            let mut g = app.ui.grid_mm;
            let step = app.i18n.t("field-grid");
            if crate::inspector::field(ui, &step, &mut g, Some(units), "", 120.0).changed() {
                app.ui.grid_mm = g.clamp(0.1, 50.0);
            }
        });
        let t = app.i18n.t("action-snap");
        setting_row(ui, &t, "", false, |ui| {
            crate::inspector::switch(ui, &mut app.ui.snap).on_hover_text(t.clone());
        });
    });
    settings_card(app, ui, "agents", |app, ui| {
        let (t, hint) = (app.i18n.t("settings-mcp"), app.i18n.t("mcp-on-hint"));
        let url = app.mcp.as_ref().map(|h| h.url.clone());
        setting_row(ui, &t, &hint, true, |ui| {
            let (text, fg, bg) = match &url {
                Some(_) => (
                    app.i18n.t("settings-mcp-on"),
                    theme::GREEN,
                    theme::GREEN_TINT,
                ),
                None => (app.i18n.t("mcp-off"), theme::MUTED, theme::BG),
            };
            egui::Frame::new()
                .fill(bg)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(9, 3))
                .show(ui, |ui| ui.label(RichText::new(text).size(12.0).color(fg)));
        });
        if let Some(url) = url {
            let (t, hint) = (
                app.i18n.t("settings-address"),
                app.i18n.t("settings-address-hint"),
            );
            setting_row(ui, &t, &hint, false, |ui| {
                if icons::button_sized(ui, "copy", &app.i18n.t("settings-copy"), false, 26.0)
                    .clicked()
                {
                    ui.ctx().copy_text(url.clone());
                }
                ui.label(RichText::new(&url).monospace().size(12.0));
            });
        }
        let (t, hint) = (
            app.i18n.t("printers-agents"),
            app.i18n.t("printers-agents-hint"),
        );
        setting_row(ui, &t, &hint, false, |ui| {
            crate::inspector::switch(ui, &mut app.ui.allow_agent_print)
                .on_hover_text(app.i18n.t("action-allow-agent-print"));
        });
        let roots: Vec<String> = app
            .agent_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let (t, hint) = (
            app.i18n.t("settings-roots"),
            app.i18n.t("settings-roots-hint"),
        );
        setting_row(ui, &t, &hint, false, |ui| {
            ui.vertical(|ui| {
                for r in &roots {
                    ui.add(egui::Label::new(RichText::new(r).size(12.0)).truncate());
                }
            });
        });
    });
    settings_card(app, ui, "about", |app, ui| {
        let t = format!("Labely {}", env!("CARGO_PKG_VERSION"));
        let hint = app.i18n.t("about-text");
        setting_row(ui, &t, &hint, true, |ui| {
            if ui
                .add(egui::Button::new(app.i18n.t("settings-about-more")).min_size(vec2(0.0, 30.0)))
                .clicked()
            {
                app.ui.dialog = Some(crate::app::Dialog::About);
            }
        });
    });
}

/// Templates pane of the editor's left rail: click a card to open it in a new tab.
pub fn templates_pane(app: &mut LabelyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(app.i18n.t("rail-templates"))
                .font(theme::bold(16.0))
                .color(theme::TEXT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tip = app.i18n.t("templates-title");
            if crate::icons::button_sized(ui, "folder", &tip, false, 26.0).clicked() {
                menu::invoke(app, "template.new");
            }
        });
    });
    ui.add_space(6.0);
    let mut query = app.ui.template_query.clone();
    ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text(app.i18n.t("templates-search"))
            .desired_width(f32::INFINITY),
    );
    app.ui.template_query = query;
    ui.add_space(4.0);
    let tabs = [
        (app.i18n.t("templates-starters-short"), None),
        (app.i18n.t("templates-mine-short"), None),
    ];
    let own = app.ui.templates_own;
    if let Some(i) = crate::inspector::segmented(ui, &tabs, usize::from(own)) {
        app.ui.templates_own = i == 1;
    }
    ui.add_space(6.0);
    let q = app.ui.template_query.to_lowercase();
    let all: Vec<_> = cached_templates(app)
        .into_iter()
        .filter(|t| t.starter != own)
        .filter(|t| q.is_empty() || templates::display_name(app, t).to_lowercase().contains(&q))
        .collect();
    let mut open = None;
    let footer = 52.0;
    egui::ScrollArea::vertical()
        .id_salt("pane-templates")
        .max_height((ui.available_height() - footer).max(60.0))
        .show(ui, |ui| {
            if all.is_empty() {
                ui.label(RichText::new(app.i18n.t("templates-empty")).color(theme::MUTED));
            }
            let gap = 8.0;
            let w = (ui.available_width() - gap) / 2.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(gap, gap);
                for t in &all {
                    if template_card(app, ui, t, w).clicked() {
                        open = Some(t.reference.clone());
                    }
                }
            });
        });
    at_bottom(ui, "pane-templates-footer", |ui| {
        ui.separator();
        ui.vertical_centered(|ui| {
            let has_doc = app.session.active_doc_id().is_some();
            ui.add_enabled_ui(has_doc, |ui| {
                if crate::icons::text_button(
                    ui,
                    "plus",
                    &menu::command_label(&app.i18n, "template.save"),
                    None,
                    theme::TEXT,
                    None,
                )
                .clicked()
                {
                    menu::invoke(app, "template.save");
                }
            });
        });
    });
    if let Some(r) = open {
        templates::open_template(app, &r);
    }
}
