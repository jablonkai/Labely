//! Print queue section of the printers panel, and the print-history dialog (ADR-0019).

use egui::{Color32, CornerRadius, Margin, RichText, Stroke, Ui, vec2};
use labely_core::jobs::{JobInfo, JobState};
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::{Dialog, LabelyApp};
use crate::{icons, theme};

/// Jobs shown in the panel (newest first).
const SHOWN: usize = 20;

fn state_key(s: JobState) -> &'static str {
    match s {
        JobState::Queued => "job-queued",
        JobState::Printing => "job-printing",
        JobState::Done => "job-done",
        JobState::Failed => "job-failed",
        JobState::Cancelled => "job-cancelled",
    }
}

/// Report jobs that finished since the last frame in the status bar; keep repainting while
/// the queue is busy so progress moves.
pub fn watch(app: &mut LabelyApp, ctx: &egui::Context) {
    let jobs = app.session.jobs().list();
    for j in &jobs {
        let before = app.ui.jobs_seen.insert(j.id.clone(), j.state);
        if before.is_none_or(|b| b == j.state) || !j.state.is_finished() {
            continue;
        }
        match j.state {
            JobState::Done => {
                let text = app
                    .i18n
                    .t_args("status-job-done", &[("name", Arg::Str(&j.doc_name))]);
                app.set_status(text, false);
            }
            JobState::Failed => {
                let why = j.error.as_ref().map_or("", |e| e.message.as_str());
                let text = format!("{}: {why}", app.i18n.t("job-failed"));
                app.set_status(text, true);
            }
            _ => {}
        }
    }
    app.ui
        .jobs_seen
        .retain(|id, _| jobs.iter().any(|j| &j.id == id));
    if app.session.jobs().is_busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

/// Jobs queued or printing, for the status bar; `None` when the queue is idle.
pub fn active_count(app: &LabelyApp) -> Option<usize> {
    let n = app
        .session
        .jobs()
        .list()
        .iter()
        .filter(|j| !j.state.is_finished())
        .count();
    (n > 0).then_some(n)
}

/// The queue: pause/resume, clear, history, and one row per job.
pub fn queue_section(app: &mut LabelyApp, ui: &mut Ui) {
    let paused = app.session.jobs().is_paused();
    let jobs: Vec<JobInfo> = app.session.jobs().list();
    ui.horizontal(|ui| {
        ui.strong(app.i18n.t("panel-jobs"));
        let (key, cmd) = if paused {
            ("action-resume", "job.resume")
        } else {
            ("action-pause", "job.pause")
        };
        if ui.small_button(app.i18n.t(key)).clicked() {
            app.run_reporting(cmd, json!({}));
        }
        if jobs.iter().any(|j| j.state.is_finished())
            && ui
                .small_button(app.i18n.t("action-clear-finished"))
                .clicked()
        {
            app.run_reporting("job.clear", json!({}));
        }
        if ui
            .small_button(crate::menu::command_label(&app.i18n, "job.history"))
            .clicked()
        {
            open_history(app);
        }
    });
    if paused {
        ui.label(RichText::new(app.i18n.t("jobs-paused")).weak());
    }
    if jobs.is_empty() {
        ui.label(RichText::new(app.i18n.t("jobs-none")).weak());
        return;
    }
    for j in jobs.iter().rev().take(SHOWN) {
        ui.horizontal(|ui| {
            let state = app.i18n.t(state_key(j.state));
            let text = format!(
                "{} · {} · {}/{} · {state}",
                j.id, j.doc_name, j.pages_done, j.pages_total
            );
            let label = if j.state == JobState::Failed {
                ui.colored_label(Color32::from_rgb(200, 60, 60), text)
            } else {
                ui.label(text)
            };
            if let Some(e) = &j.error {
                label.on_hover_text(&e.message);
            }
            if j.state == JobState::Printing && j.pages_total > 0 {
                let f = j.pages_done as f32 / j.pages_total as f32;
                ui.add(egui::ProgressBar::new(f).desired_width(80.0));
            }
            if !j.state.is_finished() && ui.small_button(app.i18n.t("action-cancel")).clicked() {
                app.run_reporting("job.cancel", json!({"id": j.id}));
            }
        });
    }
}

/// Print-history page state: entries as returned by `job.history`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryView {
    pub entries: Vec<Value>,
    pub total: u64,
    pub query: String,
}

impl HistoryView {
    pub fn load(app: &mut LabelyApp) -> Self {
        let env = app.run_reporting("job.history", json!({"limit": 1000}));
        let r = env.result.unwrap_or(Value::Null);
        Self {
            entries: r["entries"].as_array().cloned().unwrap_or_default(),
            total: r["total"].as_u64().unwrap_or(0),
            query: String::new(),
        }
    }
}

/// Show the history page of the home screen.
pub fn open_history(app: &mut LabelyApp) {
    app.ui.history = Some(HistoryView::load(app));
    app.ui.home_page = crate::home::Page::History;
    app.ui.home = true;
}

/// `2026-11-16T14:02:11Z` → `2026-11-16 14:02`.
fn short_time(s: &str) -> String {
    let s = s.replace('T', " ");
    s.chars().take(16).collect()
}

/// Status badge: tinted pill with an icon.
fn badge(ui: &mut Ui, icon: &str, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icons::show(ui, icon, 13.0, fg);
                ui.label(RichText::new(text).size(12.0).color(fg));
            });
        });
}

/// The job being printed now, as a banner with progress, pause and cancel.
fn active_banner(app: &mut LabelyApp, ui: &mut Ui) {
    let jobs = app.session.jobs().list();
    let Some(j) = jobs.iter().find(|j| !j.state.is_finished()) else {
        return;
    };
    let paused = app.session.jobs().is_paused();
    egui::Frame::new()
        .fill(theme::PRIMARY_TINT)
        .stroke(Stroke::new(1.0, theme::PRIMARY_TINT2))
        .corner_radius(CornerRadius::same(theme::RADIUS_LG))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(r, CornerRadius::same(theme::RADIUS), theme::PRIMARY);
                icons::paint(ui.painter(), r.shrink(10.0), "printer", Color32::WHITE, 1.6);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 230.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t(state_key(j.state))).font(theme::bold(14.0)),
                        );
                        ui.label(RichText::new(&j.doc_name).size(12.5).color(theme::TEXT2));
                        theme::trailing(ui, |ui| {
                            ui.label(
                                RichText::new(app.i18n.t_args(
                                    "history-progress",
                                    &[
                                        ("done", Arg::Num(f64::from(j.pages_done))),
                                        ("total", Arg::Num(f64::from(j.pages_total))),
                                    ],
                                ))
                                .font(theme::bold(12.5)),
                            );
                        });
                    });
                    let f = if j.pages_total > 0 {
                        j.pages_done as f32 / j.pages_total as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        egui::ProgressBar::new(f)
                            .desired_height(5.0)
                            .fill(theme::PRIMARY)
                            .corner_radius(CornerRadius::same(3)),
                    );
                });
                theme::trailing(ui, |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(app.i18n.t("action-cancel")).color(theme::PRIMARY),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        app.run_reporting("job.cancel", json!({"id": j.id}));
                    }
                    let (key, cmd, icon) = if paused {
                        ("action-resume", "job.resume", "play")
                    } else {
                        ("action-pause", "job.pause", "pause")
                    };
                    if icons::text_button(ui, icon, &app.i18n.t(key), None, theme::TEXT, None)
                        .clicked()
                    {
                        app.run_reporting(cmd, json!({}));
                    }
                });
            });
        });
    ui.add_space(12.0);
}

/// The print-history page (design screen 9).
pub fn history_page(app: &mut LabelyApp, ui: &mut Ui) {
    let mut view = match app.ui.history.take() {
        Some(v) => v,
        None => HistoryView::load(app),
    };
    let mut reload = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(app.i18n.t("history-title"))
                    .font(theme::bold(25.0))
                    .color(theme::TEXT),
            );
            ui.label(RichText::new(app.i18n.t("history-subtitle")).color(theme::MUTED));
        });
        theme::trailing(ui, |ui| {
            if !view.entries.is_empty()
                && ui
                    .add(
                        egui::Button::new(app.i18n.t("action-clear-history"))
                            .min_size(vec2(0.0, 34.0)),
                    )
                    .clicked()
                && app.run_reporting("job.history_clear", json!({})).ok
            {
                reload = true;
            }
            ui.add(
                egui::TextEdit::singleline(&mut view.query)
                    .hint_text(app.i18n.t("templates-search"))
                    .desired_width(220.0)
                    .min_size(vec2(0.0, 30.0)),
            );
        });
    });
    ui.add_space(14.0);
    active_banner(app, ui);
    let q = view.query.to_lowercase();
    let shown: Vec<&Value> = view
        .entries
        .iter()
        .filter(|e| {
            q.is_empty()
                || e["doc_name"]
                    .as_str()
                    .is_some_and(|n| n.to_lowercase().contains(&q))
        })
        .collect();
    let labels: f64 = shown
        .iter()
        .map(|e| e["pages_printed"].as_f64().unwrap_or(0.0))
        .sum();
    ui.horizontal(|ui| {
        theme::trailing(ui, |ui| {
            ui.label(
                RichText::new(app.i18n.t_args(
                    "history-summary",
                    &[
                        ("count", Arg::Num(shown.len() as f64)),
                        ("labels", Arg::Num(labels)),
                    ],
                ))
                .size(12.0)
                .color(theme::MUTED),
            );
        });
    });
    ui.add_space(6.0);
    let mut open_print = false;
    theme::card().inner_margin(Margin::ZERO).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if shown.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(app.i18n.t("history-empty")).color(theme::MUTED));
            });
            ui.add_space(24.0);
            return;
        }
        let w = ui.available_width();
        let cols = [0.0, 0.30, 0.45, 0.55, 0.62];
        let header = [
            "history-col-label",
            "history-col-time",
            "history-col-printer",
            "history-col-count",
            "history-col-state",
        ];
        let (hr, _) = ui.allocate_exact_size(vec2(w, 36.0), egui::Sense::hover());
        for (c, k) in cols.iter().zip(header) {
            ui.painter().text(
                egui::pos2(hr.left() + 16.0 + c * w, hr.center().y),
                egui::Align2::LEFT_CENTER,
                app.i18n.t(k),
                theme::regular(11.5),
                theme::MUTED,
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("history")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for e in shown {
                    let id = e["id"].as_str().unwrap_or("").to_owned();
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 54.0), egui::Sense::hover());
                    ui.painter().line_segment(
                        [rect.left_top(), rect.right_top()],
                        Stroke::new(1.0, theme::BORDER_SOFT),
                    );
                    let mut row = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(vec2(16.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    let x = |c: f32| rect.left() + 16.0 + c * w;
                    // Label: icon, name, labels × copies.
                    let (ir, _) = row.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                    row.painter().rect_filled(
                        ir,
                        CornerRadius::same(theme::RADIUS),
                        theme::PRIMARY_TINT,
                    );
                    icons::paint(row.painter(), ir.shrink(9.0), "file", theme::PRIMARY, 1.5);
                    row.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.set_width(x(cols[1]) - ir.right() - 24.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(e["doc_name"].as_str().unwrap_or(""))
                                    .font(theme::bold(13.0)),
                            )
                            .truncate(),
                        );
                        let model = e["model"].as_str().unwrap_or("");
                        ui.label(RichText::new(model).size(11.5).color(theme::MUTED));
                    });
                    let p = row.painter().clone();
                    let text = |c: f32, t: String, bold: bool| {
                        p.text(
                            egui::pos2(x(c), rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            t,
                            if bold {
                                theme::bold(12.5)
                            } else {
                                theme::regular(12.5)
                            },
                            theme::TEXT,
                        );
                    };
                    text(
                        cols[1],
                        short_time(e["printed_at"].as_str().unwrap_or("")),
                        false,
                    );
                    let printer = e["address"].as_str().unwrap_or("").to_owned();
                    let printer = printer.split(':').next().unwrap_or("").to_uppercase();
                    text(cols[2], printer, false);
                    let done = e["pages_printed"].as_u64().unwrap_or(0);
                    let total = u64::from(e["labels"].as_u64().unwrap_or(0) as u32)
                        * e["copies"].as_u64().unwrap_or(1).max(1);
                    let count = if total > done && total > 0 {
                        format!("{done} / {total}")
                    } else {
                        done.to_string()
                    };
                    text(cols[3], count, true);
                    let state = e["state"].as_str().unwrap_or("done").to_owned();
                    let mut badge_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(egui::Rect::from_min_max(
                                egui::pos2(x(cols[4]), rect.top()),
                                rect.right_bottom(),
                            ))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    match state.as_str() {
                        "failed" => badge(
                            &mut badge_ui,
                            "warn",
                            &app.i18n.t("job-failed"),
                            theme::RED,
                            Color32::from_rgb(0xFF, 0xF0, 0xF0),
                        ),
                        "cancelled" => badge(
                            &mut badge_ui,
                            "pause",
                            &app.i18n.t("job-cancelled"),
                            theme::AMBER,
                            theme::AMBER_TINT,
                        ),
                        _ => badge(
                            &mut badge_ui,
                            "check",
                            &app.i18n.t("job-done"),
                            theme::GREEN,
                            theme::GREEN_TINT,
                        ),
                    }
                    if let Some(err) = e["error"].as_str() {
                        badge_ui
                            .label(RichText::new("ⓘ").color(theme::MUTED))
                            .on_hover_text(err);
                    }
                    let mut actions = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(vec2(12.0, 0.0)))
                            .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    );
                    let has_file = e["file"].is_string();
                    actions.menu_button(RichText::new("···").color(theme::MUTED), |ui| {
                        if has_file
                            && ui.button(app.i18n.t("templates-open")).clicked()
                            && app.run_reporting("job.history_open", json!({"id": id})).ok
                        {
                            app.ui.fit_pending = true;
                            ui.close();
                        }
                        if ui.button(app.i18n.t("templates-delete")).clicked()
                            && app
                                .run_reporting("job.history_delete", json!({"ids": [id]}))
                                .ok
                        {
                            reload = true;
                            ui.close();
                        }
                    });
                    if has_file {
                        let key = if state == "done" {
                            "history-reprint"
                        } else {
                            "history-continue"
                        };
                        if icons::text_button(
                            &mut actions,
                            "printer",
                            &app.i18n.t(key),
                            None,
                            theme::TEXT,
                            None,
                        )
                        .clicked()
                            && app.run_reporting("job.history_open", json!({"id": id})).ok
                        {
                            app.ui.fit_pending = true;
                            open_print = true;
                        }
                    }
                }
            });
    });
    if open_print {
        app.ui.dialog = Some(Dialog::Print);
    }
    if reload {
        let q = std::mem::take(&mut view.query);
        view = HistoryView::load(app);
        view.query = q;
    }
    app.ui.history = Some(view);
}
