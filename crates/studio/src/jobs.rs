//! Print queue section of the printers panel, and the print-history dialog (ADR-0019).

use egui::{Color32, RichText, Ui};
use labely_core::jobs::{JobInfo, JobState};
use labely_i18n::Arg;
use serde_json::{Value, json};

use crate::app::{Dialog, LabelyApp};

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

/// Print-history dialog state: entries as returned by `job.history`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryView {
    pub entries: Vec<Value>,
    pub total: u64,
}

impl HistoryView {
    fn load(app: &mut LabelyApp) -> Self {
        let env = app.run_reporting("job.history", json!({"limit": 1000}));
        let r = env.result.unwrap_or(Value::Null);
        Self {
            entries: r["entries"].as_array().cloned().unwrap_or_default(),
            total: r["total"].as_u64().unwrap_or(0),
        }
    }
}

pub fn open_history(app: &mut LabelyApp) {
    let view = HistoryView::load(app);
    app.ui.dialog = Some(Dialog::History(view));
}

/// Returns true when the dialog should close.
pub fn history(app: &mut LabelyApp, ui: &mut Ui, view: &mut HistoryView) -> bool {
    ui.heading(app.i18n.t("history-title"));
    let mut close = false;
    let mut reload = false;
    if view.entries.is_empty() {
        ui.label(RichText::new(app.i18n.t("history-empty")).weak());
    }
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .id_salt("history")
        .show(ui, |ui| {
            egui::Grid::new("history-grid")
                .num_columns(5)
                .striped(true)
                .show(ui, |ui| {
                    for e in &view.entries {
                        let id = e["id"].as_str().unwrap_or("").to_owned();
                        ui.label(e["printed_at"].as_str().unwrap_or(""));
                        ui.label(e["doc_name"].as_str().unwrap_or(""));
                        ui.label(app.i18n.t_args(
                            "history-pages",
                            &[(
                                "count",
                                Arg::Num(e["pages_printed"].as_f64().unwrap_or(0.0)),
                            )],
                        ));
                        let state = match e["state"].as_str() {
                            Some("failed") => app.i18n.t("job-failed"),
                            Some("cancelled") => app.i18n.t("job-cancelled"),
                            _ => app.i18n.t("job-done"),
                        };
                        let l = ui.label(state);
                        if let Some(err) = e["error"].as_str() {
                            l.on_hover_text(err);
                        }
                        ui.horizontal(|ui| {
                            if e["file"].is_string()
                                && ui.small_button(app.i18n.t("templates-open")).clicked()
                                && app.run_reporting("job.history_open", json!({"id": id})).ok
                            {
                                app.ui.fit_pending = true;
                                close = true;
                            }
                            if ui.small_button(app.i18n.t("templates-delete")).clicked()
                                && app
                                    .run_reporting("job.history_delete", json!({"ids": [id]}))
                                    .ok
                            {
                                reload = true;
                            }
                        });
                        ui.end_row();
                    }
                });
        });
    ui.separator();
    ui.horizontal(|ui| {
        if !view.entries.is_empty()
            && ui.button(app.i18n.t("action-clear-history")).clicked()
            && app.run_reporting("job.history_clear", json!({})).ok
        {
            reload = true;
        }
        if ui.button(app.i18n.t("action-close")).clicked() {
            close = true;
        }
    });
    if reload {
        *view = HistoryView::load(app);
    }
    close
}
