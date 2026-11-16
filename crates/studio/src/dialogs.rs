//! Modal dialogs: new document, templates, print, about.

use egui::{Color32, Context, CornerRadius, Margin, Modal, RichText, Sense, Stroke, Ui, vec2};
use labely_i18n::Arg;
use serde_json::json;

use crate::app::{Dialog, LabelyApp};
use crate::{icons, theme};

/// `doc.new` form.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDocForm {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub printer_model: String,
    /// `MediaType` in snake case.
    pub media: String,
}

impl Default for NewDocForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            width_mm: 50.0,
            height_mm: 30.0,
            printer_model: "niimbot.b21_pro".to_owned(),
            media: "gap".to_owned(),
        }
    }
}

/// Print dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintForm {
    pub address: String,
    pub copies: u16,
    /// 0 = model default.
    pub density: u8,
    /// Data records, e.g. `1,3,5-7`; empty = all.
    pub records: String,
    /// Labels to print without data.
    pub count: u32,
    /// Print offset in mm, right and down.
    pub offset_x_mm: f32,
    pub offset_y_mm: f32,
    /// Mirror the print left to right.
    pub mirror: bool,
    /// Turn the print 180°.
    pub rotate_180: bool,
    /// Data column holding the copies of each record; empty = `copies` for every record.
    pub copies_column: String,
}

impl Default for PrintForm {
    fn default() -> Self {
        let dir = std::env::temp_dir().join("labely-fake-printer");
        Self {
            address: format!("fake:{}", dir.display()),
            copies: 1,
            density: 0,
            records: String::new(),
            count: 1,
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            mirror: false,
            rotate_180: false,
            copies_column: String::new(),
        }
    }
}

/// Export dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportForm {
    pub pdf: bool,
    pub dpi: u32,
    /// Every record instead of the current one.
    pub all: bool,
    /// Target file; `{record}` is replaced by the record number for PNG.
    pub path: String,
}

impl ExportForm {
    pub fn new(app: &LabelyApp, pdf: bool) -> Self {
        let path = app
            .session
            .doc_path(None)
            .map(|p| {
                let mut p = p.to_path_buf();
                p.set_extension(if pdf { "pdf" } else { "png" });
                p.display().to_string()
            })
            .unwrap_or_default();
        Self {
            pdf,
            dpi: 300,
            all: false,
            path,
        }
    }

    /// Path with the right extension; for several PNG files, `{record}` (or a `-001` suffix)
    /// numbers them.
    fn target(&self, record: Option<usize>) -> String {
        let ext = if self.pdf { "pdf" } else { "png" };
        let mut p = std::path::PathBuf::from(self.path.trim());
        if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case(ext)) {
            p.set_extension(ext);
        }
        let s = p.display().to_string();
        match record {
            None => s.replace("{record}", ""),
            Some(r) if s.contains("{record}") => s.replace("{record}", &format!("{r:03}")),
            Some(r) => {
                let stem = p
                    .file_stem()
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_default();
                p.with_file_name(format!("{stem}-{r:03}.{ext}"))
                    .display()
                    .to_string()
            }
        }
    }
}

/// Show the open dialog, if any.
pub fn show(app: &mut LabelyApp, ctx: &Context) {
    let Some(dialog) = app.ui.dialog.clone() else {
        return;
    };
    let mut close = false;
    let mut next = dialog.clone();
    // One id per dialog kind, so each sizes itself instead of inheriting another's size.
    let kind = std::mem::discriminant(&dialog);
    Modal::new(egui::Id::unique(("labely-dialog", kind))).show(ctx, |ui| {
        ui.set_min_width(320.0);
        close = match &mut next {
            Dialog::NewDoc(form) => new_doc(app, ui, form),
            Dialog::SaveTemplate(f) => crate::templates::save(app, ui, f),
            Dialog::Papers(v) => crate::paper::library(app, ui, v),
            Dialog::Device(v) => crate::paper::device(app, ui, v),
            Dialog::Print => print(app, ui),
            Dialog::Export(f) => export(app, ui, f),
            Dialog::About => about(app, ui),
            Dialog::ConfirmClose(doc_id) => confirm_close(app, ui, doc_id),
        };
    });
    app.ui.dialog = if close {
        app.ui.dialog_next.take()
    } else {
        Some(next)
    };
}

/// Dialog title with a close button; returns true when closed.
fn dialog_header(app: &LabelyApp, ui: &mut Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::bold(17.0))
                .color(theme::TEXT),
        );
        theme::trailing(ui, |ui| {
            close = icons::button(ui, "close", &app.i18n.t("action-cancel"), false).clicked();
        });
    });
    ui.add_space(10.0);
    close
}

/// Label outline with dimension lines, as in the new-label preview.
fn size_preview(ui: &mut Ui, w_mm: f64, h_mm: f64, area: egui::Vec2, fill: Color32) {
    let (rect, _) = ui.allocate_exact_size(area, Sense::hover());
    let p = ui.painter();
    let (w, h) = (w_mm.max(1.0) as f32, h_mm.max(1.0) as f32);
    let scale = ((area.x - 70.0) / w).min((area.y - 60.0) / h);
    let label = egui::Rect::from_center_size(rect.center() + vec2(10.0, 10.0), vec2(w, h) * scale);
    let line = Stroke::new(1.0, theme::TEXT2);
    let top = label.top() - 18.0;
    p.line_segment(
        [
            egui::pos2(label.left(), top),
            egui::pos2(label.right(), top),
        ],
        line,
    );
    for x in [label.left(), label.right()] {
        p.line_segment([egui::pos2(x, top - 4.0), egui::pos2(x, top + 4.0)], line);
    }
    p.text(
        egui::pos2(label.center().x, top - 6.0),
        egui::Align2::CENTER_BOTTOM,
        format!("{} mm", crate::home::mm(w_mm)),
        theme::regular(11.5),
        theme::TEXT2,
    );
    let left = label.left() - 18.0;
    p.line_segment(
        [
            egui::pos2(left, label.top()),
            egui::pos2(left, label.bottom()),
        ],
        line,
    );
    for y in [label.top(), label.bottom()] {
        p.line_segment([egui::pos2(left - 4.0, y), egui::pos2(left + 4.0, y)], line);
    }
    p.text(
        egui::pos2(left - 8.0, label.center().y),
        egui::Align2::RIGHT_CENTER,
        crate::home::mm(h_mm),
        theme::regular(11.5),
        theme::TEXT2,
    );
    p.add(egui::Shape::rect_filled(
        label.translate(vec2(0.0, 3.0)),
        CornerRadius::same(8),
        Color32::from_black_alpha(18),
    ));
    p.rect(
        label,
        CornerRadius::same(8),
        fill,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let inner = label.shrink(6.0);
    let pts = [
        inner.left_top(),
        inner.right_top(),
        inner.right_bottom(),
        inner.left_bottom(),
        inner.left_top(),
    ];
    p.add(egui::Shape::dashed_line(
        &pts,
        Stroke::new(1.0, theme::PRIMARY_TINT2),
        4.0,
        3.0,
    ));
}

/// Returns true when the dialog should close.
fn new_doc(app: &mut LabelyApp, ui: &mut Ui, f: &mut NewDocForm) -> bool {
    ui.set_width(760.0);
    let title = app.i18n.t("cmd-doc-new");
    let mut close = dialog_header(app, ui, title.trim_end_matches(['…', '.']));
    let units = app.units();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(450.0, 360.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(450.0);
                ui.label(RichText::new(app.i18n.t("newdoc-size")).font(theme::bold(12.5)));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (w, h) in crate::home::QUICK_SIZES {
                        let active = f.width_mm == w && f.height_mm == h;
                        let text = format!("{} × {} mm", crate::home::mm(w), crate::home::mm(h));
                        if crate::home::chip(ui, &text, active).clicked() {
                            f.width_mm = w;
                            f.height_mm = h;
                        }
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let w = 130.0;
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t("prop-width-mm")).font(theme::bold(12.5)),
                        );
                        let pre = app.i18n.t("field-width");
                        let r =
                            crate::inspector::field(ui, &pre, &mut f.width_mm, Some(units), "", w);
                        r.on_hover_text(app.i18n.t("prop-width-mm"));
                    });
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t("prop-height-mm")).font(theme::bold(12.5)),
                        );
                        let pre = app.i18n.t("field-height");
                        let r =
                            crate::inspector::field(ui, &pre, &mut f.height_mm, Some(units), "", w);
                        r.on_hover_text(app.i18n.t("prop-height-mm"));
                    });
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(app.i18n.t("newdoc-orientation")).font(theme::bold(12.5)),
                        );
                        let items = [
                            (app.i18n.t("orientation-landscape"), None),
                            (app.i18n.t("orientation-portrait"), None),
                        ];
                        let portrait = f.height_mm > f.width_mm;
                        if let Some(i) =
                            crate::inspector::segmented(ui, &items, usize::from(portrait))
                            && (i == 1) != portrait
                        {
                            std::mem::swap(&mut f.width_mm, &mut f.height_mm);
                        }
                    });
                });
                f.width_mm = f.width_mm.clamp(5.0, 300.0);
                f.height_mm = f.height_mm.clamp(5.0, 1000.0);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let l = ui
                            .label(RichText::new(app.i18n.t("prop-media")).font(theme::bold(12.5)))
                            .id;
                        crate::inspector::media_combo(app, ui, "new_media", &mut f.media, 190.0)
                            .labelled_by(l);
                    });
                    ui.vertical(|ui| {
                        let l = ui
                            .label(
                                RichText::new(app.i18n.t("prop-printer-model"))
                                    .font(theme::bold(12.5)),
                            )
                            .id;
                        let models = app.session.services().devices.models();
                        let shown = models
                            .iter()
                            .find(|m| m.id == f.printer_model)
                            .map_or_else(|| app.i18n.t("model-none"), |m| m.name.clone());
                        egui::ComboBox::from_id_salt("new_model")
                            .selected_text(shown)
                            .width(190.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut f.printer_model,
                                    String::new(),
                                    app.i18n.t("model-none"),
                                );
                                for m in &models {
                                    ui.selectable_value(
                                        &mut f.printer_model,
                                        m.id.clone(),
                                        &m.name,
                                    );
                                }
                            })
                            .response
                            .labelled_by(l);
                    });
                });
                ui.add_space(10.0);
                ui.label(RichText::new(app.i18n.t("newdoc-color")).font(theme::bold(12.5)));
                ui.allocate_ui(vec2(300.0, 30.0), |ui| {
                    crate::inspector::label_colors(app, ui)
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let l = ui
                        .label(RichText::new(app.i18n.t("prop-name")).font(theme::bold(12.5)))
                        .id;
                    ui.add(
                        egui::TextEdit::singleline(&mut f.name)
                            .hint_text(app.i18n.t("newdoc-name-hint"))
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(l);
                });
            },
        );
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 360.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let [r, g, b] = app.ui.label_color;
                let fill = Color32::from_rgb(r, g, b);
                size_preview(
                    ui,
                    f.width_mm,
                    f.height_mm,
                    vec2(ui.available_width(), 200.0),
                    fill,
                );
                ui.add_space(8.0);
                let connected = app.session.services().devices.connected();
                if let Some(d) = connected.first() {
                    theme::card().inner_margin(Margin::same(12)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let name = d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.0, theme::GREEN);
                            ui.label(
                                RichText::new(
                                    app.i18n
                                        .t_args("newdoc-connected", &[("name", Arg::Str(&name))]),
                                )
                                .font(theme::bold(12.5)),
                            );
                        });
                        let paper = app
                            .ui
                            .detected
                            .as_ref()
                            .and_then(|d| d.get("paper"))
                            .and_then(|p| {
                                Some((p["width_mm"].as_f64()?, p["height_mm"].as_f64()?))
                            });
                        match paper {
                            Some((w, h)) => {
                                ui.label(
                                    RichText::new(app.i18n.t_args(
                                        "newdoc-loaded",
                                        &[("width", Arg::Num(w)), ("height", Arg::Num(h))],
                                    ))
                                    .size(11.5)
                                    .color(theme::TEXT2),
                                );
                                if crate::home::chip(ui, &app.i18n.t("newdoc-use-size"), true)
                                    .clicked()
                                {
                                    f.width_mm = w;
                                    f.height_mm = h;
                                }
                            }
                            None => {
                                if crate::home::chip(ui, &app.i18n.t("newdoc-read-paper"), false)
                                    .clicked()
                                {
                                    crate::paper::detect(app, Some(&d.address));
                                    if app.ui.detected.is_none() {
                                        app.set_status(app.i18n.t("status-no-rfid"), false);
                                    }
                                }
                            }
                        }
                    });
                }
            },
        );
    });
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        if icons::text_button(
            ui,
            "grid",
            &app.i18n.t("newdoc-from-template"),
            None,
            theme::PRIMARY,
            None,
        )
        .clicked()
        {
            crate::templates::open_library(app);
            close = true;
        }
        theme::trailing(ui, |ui| {
            if ui
                .add(theme::primary_button(app.i18n.t("action-create")))
                .clicked()
            {
                let mut p = json!({
                    "width_mm": f.width_mm, "height_mm": f.height_mm, "media": f.media,
                });
                if !f.name.trim().is_empty() {
                    p["name"] = json!(f.name.trim());
                }
                if !f.printer_model.is_empty() {
                    p["printer_model"] = json!(f.printer_model);
                }
                if app.run_reporting("doc.new", p).ok {
                    app.ui.fit_pending = true;
                    close = true;
                }
            }
            if ui
                .add(egui::Button::new(app.i18n.t("action-cancel")).min_size(vec2(0.0, 32.0)))
                .clicked()
            {
                close = true;
            }
        });
    });
    close
}

/// Export to PNG or PDF (design screen 12).
fn export(app: &mut LabelyApp, ui: &mut Ui, f: &mut ExportForm) -> bool {
    ui.set_width(500.0);
    let title = app.i18n.t("export-title");
    let mut close = dialog_header(app, ui, &title);
    let records = app
        .session
        .doc(None)
        .and_then(|d| d.data.as_ref().map(|d| d.rows.len()))
        .unwrap_or(0);
    let bold = |t: String| RichText::new(t).font(theme::bold(12.5));
    let full = |ui: &mut Ui, items: &[(String, Option<&str>)], sel: usize| {
        ui.scope(|ui| {
            ui.set_width(ui.available_width());
            crate::inspector::segmented(ui, items, sel)
        })
        .inner
    };
    ui.label(bold(app.i18n.t("export-format")));
    if let Some(i) = full(
        ui,
        &[("PNG".into(), None), ("PDF".into(), None)],
        usize::from(f.pdf),
    ) {
        f.pdf = i == 1;
    }
    ui.add_space(8.0);
    ui.label(bold(app.i18n.t("export-resolution")));
    const DPIS: [u32; 3] = [203, 300, 600];
    let items: Vec<_> = DPIS.iter().map(|d| (format!("{d} dpi"), None)).collect();
    let sel = DPIS.iter().position(|d| *d == f.dpi).unwrap_or(0);
    if let Some(i) = full(ui, &items, sel) {
        f.dpi = DPIS[i];
    }
    if records > 0 {
        ui.add_space(8.0);
        ui.label(bold(app.i18n.t("export-records")));
        let items = [
            (app.i18n.t("export-current"), None),
            (
                app.i18n
                    .t_args("export-all", &[("count", Arg::Num(records as f64))]),
                None,
            ),
        ];
        if let Some(i) = full(ui, &items, usize::from(f.all)) {
            f.all = i == 1;
        }
    }
    ui.add_space(8.0);
    let l = ui.label(bold(app.i18n.t("export-path"))).id;
    ui.horizontal(|ui| {
        let w = ui.available_width() - 40.0;
        ui.add(
            egui::TextEdit::singleline(&mut f.path)
                .hint_text(app.i18n.t("export-path-hint"))
                .desired_width(w),
        )
        .labelled_by(l);
        if icons::button(ui, "folder", &app.i18n.t("export-browse"), false).clicked() {
            browse_export(app, f);
        }
    });
    let many_png = !f.pdf && f.all && records > 1;
    if many_png && !f.path.trim().is_empty() {
        ui.label(
            RichText::new(app.i18n.t_args(
                "export-files",
                &[
                    ("count", Arg::Num(records as f64)),
                    ("first", Arg::Str(&file_name(&f.target(Some(1))))),
                    ("last", Arg::Str(&file_name(&f.target(Some(records))))),
                ],
            ))
            .size(11.5)
            .color(theme::MUTED),
        );
    }
    ui.add_space(10.0);
    ui.separator();
    ui.horizontal(|ui| {
        theme::trailing(ui, |ui| {
            let go = icons::text_button(
                ui,
                "download",
                &app.i18n.t("export-action"),
                Some(theme::PRIMARY),
                Color32::WHITE,
                None,
            );
            if go.clicked() {
                if f.path.trim().is_empty() {
                    browse_export(app, f);
                }
                if !f.path.trim().is_empty() && run_export(app, f, records) {
                    close = true;
                }
            }
            if ui
                .add(egui::Button::new(app.i18n.t("action-cancel")).min_size(vec2(0.0, 32.0)))
                .clicked()
            {
                close = true;
            }
        });
    });
    close
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Native save dialog for the export target.
fn browse_export(app: &mut LabelyApp, f: &mut ExportForm) {
    if !app.opts.native_dialogs {
        app.set_status(app.i18n.t("status-no-dialogs"), true);
        return;
    }
    let ext = if f.pdf { "pdf" } else { "png" };
    let name = app
        .session
        .doc(None)
        .map_or_else(|| "label".to_owned(), |d| d.name.clone());
    if let Some(path) = rfd::FileDialog::new()
        .add_filter(ext.to_uppercase(), &[ext])
        .set_file_name(format!("{name}.{ext}"))
        .save_file()
    {
        f.path = path.display().to_string();
    }
}

/// Run the export commands; true when all succeeded.
fn run_export(app: &mut LabelyApp, f: &ExportForm, records: usize) -> bool {
    let ok = if f.pdf {
        let mut p = json!({"path": f.target(None), "dpi": f.dpi});
        if records > 0 && !f.all {
            p["records"] = json!(app.ui.record.clamp(1, records).to_string());
        }
        app.run_reporting("export.pdf", p).ok
    } else if f.all && records > 1 {
        (1..=records).all(|r| {
            let p = json!({"path": f.target(Some(r)), "dpi": f.dpi, "record": r});
            app.run_reporting("export.png", p).ok
        })
    } else {
        let p = json!({"path": f.target(None), "dpi": f.dpi, "record": app.ui.record.max(1)});
        app.run_reporting("export.png", p).ok
    };
    if ok {
        app.set_status(app.i18n.t("status-exported"), false);
    }
    ok
}

/// Record numbers picked by a `1,3,5-7` list (1-based, within `1..=n`); empty means all.
pub fn parse_records(spec: &str, n: usize) -> Vec<usize> {
    let spec = spec.trim();
    if spec.is_empty() {
        return (1..=n).collect();
    }
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse().ok(), b.trim().parse().ok()),
            None => {
                let v = part.parse().ok();
                (v, v)
            }
        };
        if let (Some(a), Some(b)) = (a, b) {
            for r in a..=b {
                if (1..=n).contains(&r) {
                    out.push(r);
                }
            }
        }
    }
    out
}

/// How many labels the form prints for `doc`.
fn total_labels(doc: &labely_core::Document, form: &PrintForm) -> u64 {
    let copies = u64::from(form.copies.max(1));
    match &doc.data {
        Some(d) => {
            let picked = parse_records(&form.records, d.rows.len());
            if form.copies_column.is_empty() {
                picked.len() as u64 * copies
            } else {
                let col = d.columns.iter().position(|c| *c == form.copies_column);
                picked
                    .iter()
                    .map(|r| {
                        col.and_then(|c| d.rows.get(r - 1).and_then(|row| row.get(c)))
                            .and_then(|v| v.trim().parse::<u64>().ok())
                            .unwrap_or(1)
                    })
                    .sum()
            }
        }
        None => u64::from(form.count.max(1)) * copies,
    }
}

/// Muted caption above a field.
fn caption(ui: &mut Ui, text: &str) -> egui::Id {
    ui.label(
        RichText::new(text)
            .font(theme::bold(12.5))
            .color(theme::TEXT),
    )
    .id
}

/// The print dialog (design screen 3): preview with record paging on the left, printer and
/// options on the right, the labels count on the print button.
fn print(app: &mut LabelyApp, ui: &mut Ui) -> bool {
    let connected = app.session.services().devices.connected();
    let mut form = app.ui.print.clone();
    let mut close = false;
    ui.set_width(820.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(app.i18n.t("menu-print"))
                .font(theme::bold(17.0))
                .color(theme::TEXT),
        );
        theme::trailing(ui, |ui| {
            if icons::button(ui, "close", &app.i18n.t("action-cancel"), false).clicked() {
                close = true;
            }
        });
    });
    ui.add_space(10.0);
    let doc = app.session.doc_arc(None);
    let records = doc
        .as_ref()
        .and_then(|d| d.data.as_ref())
        .map(|d| d.rows.len());
    ui.horizontal_top(|ui| {
        // Left: preview.
        ui.allocate_ui_with_layout(
            vec2(392.0, 400.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::Frame::new()
                    .fill(theme::BG)
                    .corner_radius(CornerRadius::same(theme::RADIUS_LG))
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(360.0);
                        ui.set_min_height(380.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(app.i18n.t("print-preview"))
                                    .font(theme::bold(13.0))
                                    .color(theme::TEXT),
                            );
                            theme::trailing(ui, |ui| {
                                ui.label(
                                    RichText::new(app.i18n.t("print-1bit"))
                                        .size(11.5)
                                        .color(theme::MUTED),
                                );
                            });
                        });
                        ui.add_space(10.0);
                        let (area, _) = ui.allocate_exact_size(vec2(328.0, 250.0), Sense::hover());
                        ui.painter().rect_filled(
                            area,
                            CornerRadius::same(theme::RADIUS),
                            theme::CANVAS,
                        );
                        print_preview(app, ui, &form, area.shrink(18.0));
                        if let Some(r) = &app.raster {
                            ui.add_space(6.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(app.i18n.t_args(
                                        "print-raster-caption",
                                        &[("dpi", Arg::Num(f64::from(r.dpi)))],
                                    ))
                                    .size(11.5)
                                    .color(theme::MUTED),
                                );
                            });
                        }
                        if let Some(n) = records.filter(|n| *n > 0) {
                            ui.add_space(8.0);
                            let mut record = app.ui.record.clamp(1, n);
                            ui.horizontal(|ui| {
                                if icons::button(
                                    ui,
                                    "chev_left",
                                    &app.i18n.t("data-record-prev"),
                                    false,
                                )
                                .clicked()
                                {
                                    record = record.saturating_sub(1).max(1);
                                }
                                theme::trailing(ui, |ui| {
                                    if icons::button(
                                        ui,
                                        "chev_right",
                                        &app.i18n.t("data-record-next"),
                                        false,
                                    )
                                    .clicked()
                                    {
                                        record = (record + 1).min(n);
                                    }
                                    ui.centered_and_justified(|ui| {
                                        ui.label(
                                            RichText::new(app.i18n.t_args(
                                                "print-record-of",
                                                &[
                                                    ("record", Arg::Num(record as f64)),
                                                    ("count", Arg::Num(n as f64)),
                                                ],
                                            ))
                                            .font(theme::bold(12.5)),
                                        );
                                    });
                                });
                            });
                            app.ui.record = record;
                        }
                    });
            },
        );
        ui.add_space(16.0);
        // Right: printer and options.
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(w, 400.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing.y = 8.0;
                let l = caption(ui, &app.i18n.t("print-printer"));
                let device = connected.iter().find(|d| d.address == form.address);
                egui::Frame::new()
                    .stroke(Stroke::new(1.5, theme::PRIMARY))
                    .corner_radius(CornerRadius::same(theme::RADIUS_LG))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                            ui.painter().rect_filled(
                                r,
                                CornerRadius::same(theme::RADIUS),
                                theme::PRIMARY_TINT,
                            );
                            icons::paint(
                                ui.painter(),
                                r.shrink(8.0),
                                "printer",
                                theme::PRIMARY,
                                1.5,
                            );
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                match device {
                                    Some(d) => {
                                        let name = d
                                            .model
                                            .as_ref()
                                            .map_or(d.name.clone(), |m| m.name.clone());
                                        ui.label(RichText::new(name).font(theme::bold(13.0)));
                                        let mut sub = app.i18n.t("home-printer-connected");
                                        if let Some(b) = crate::shell::battery(app, &d.address) {
                                            sub.push_str(&format!(" · {b}%"));
                                        }
                                        ui.label(RichText::new(sub).size(11.5).color(theme::GREEN));
                                    }
                                    None => {
                                        ui.label(
                                            RichText::new(app.i18n.t("print-address"))
                                                .font(theme::bold(13.0)),
                                        );
                                        ui.label(
                                            RichText::new(app.i18n.t("print-not-connected"))
                                                .size(11.5)
                                                .color(theme::MUTED),
                                        );
                                    }
                                }
                            });
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut form.address)
                                .desired_width(f32::INFINITY),
                        )
                        .labelled_by(l);
                        if !connected.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for d in &connected {
                                    let name =
                                        d.model.as_ref().map_or(d.name.clone(), |m| m.name.clone());
                                    if ui
                                        .selectable_label(form.address == d.address, name)
                                        .clicked()
                                    {
                                        form.address = d.address.clone();
                                    }
                                }
                            });
                        }
                    });
                ui.columns(2, |cols| {
                    let ui = &mut cols[0];
                    let l = caption(ui, &app.i18n.t("print-copies"));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if icons::button(ui, "minus", &app.i18n.t("print-fewer"), false).clicked() {
                            form.copies = form.copies.saturating_sub(1).max(1);
                        }
                        ui.add_sized(
                            [48.0, 28.0],
                            egui::DragValue::new(&mut form.copies).range(1..=999),
                        )
                        .labelled_by(l);
                        if icons::button(ui, "plus", &app.i18n.t("print-more"), false).clicked() {
                            form.copies = (form.copies + 1).min(999);
                        }
                    });
                    let ui = &mut cols[1];
                    if records.is_none() {
                        let l = caption(ui, &app.i18n.t("print-count"));
                        ui.add_sized(
                            [ui.available_width(), 28.0],
                            egui::DragValue::new(&mut form.count).range(1..=5000),
                        )
                        .labelled_by(l);
                    } else {
                        let columns: Vec<String> = doc
                            .as_ref()
                            .and_then(|d| d.data.as_ref().map(|s| s.columns.clone()))
                            .unwrap_or_default();
                        if !form.copies_column.is_empty() && !columns.contains(&form.copies_column)
                        {
                            form.copies_column.clear();
                        }
                        let l = caption(ui, &app.i18n.t("print-copies-column"));
                        let none = app.i18n.t("print-copies-column-none");
                        let shown = if form.copies_column.is_empty() {
                            none.clone()
                        } else {
                            form.copies_column.clone()
                        };
                        egui::ComboBox::from_id_salt("print-copies-column")
                            .selected_text(shown)
                            .width(ui.available_width() - 28.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut form.copies_column, String::new(), none);
                                for c in &columns {
                                    ui.selectable_value(&mut form.copies_column, c.clone(), c);
                                }
                            })
                            .response
                            .labelled_by(l)
                            .on_hover_text(app.i18n.t("print-copies-column-hint"));
                    }
                });
                let max = doc
                    .as_ref()
                    .and_then(|d| d.label.printer_model.clone())
                    .and_then(|id| {
                        app.session
                            .services()
                            .devices
                            .models()
                            .into_iter()
                            .find(|m| m.id == id)
                    })
                    .map_or(5, |m| m.density_max);
                ui.horizontal(|ui| {
                    let l = caption(ui, &app.i18n.t("prop-density"));
                    theme::trailing(ui, |ui| {
                        let shown = if form.density == 0 {
                            app.i18n.t("print-density-auto")
                        } else {
                            format!("{} / {max}", form.density)
                        };
                        ui.label(RichText::new(shown).font(theme::bold(12.5)));
                        ui.spacing_mut().slider_width = ui.available_width() - 12.0;
                        ui.add(egui::Slider::new(&mut form.density, 0..=max).show_value(false))
                            .labelled_by(l)
                            .on_hover_text(app.i18n.t("print-density-default"));
                    });
                });
                if records.is_some() {
                    let l = caption(ui, &app.i18n.t("print-records"));
                    ui.add(
                        egui::TextEdit::singleline(&mut form.records)
                            .hint_text(app.i18n.t("print-records-hint"))
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(l);
                    ui.label(
                        RichText::new(app.i18n.t("print-records-hint"))
                            .size(11.5)
                            .color(theme::MUTED),
                    );
                }
                egui::CollapsingHeader::new(app.i18n.t("print-advanced"))
                    .id_salt("print-advanced")
                    .show(ui, |ui| {
                        let l = ui.label(app.i18n.t("print-offset")).id;
                        ui.horizontal(|ui| {
                            for (v, key) in [
                                (&mut form.offset_x_mm, "print-offset-x"),
                                (&mut form.offset_y_mm, "print-offset-y"),
                            ] {
                                let mut mm = f64::from(*v);
                                let r = ui.add(
                                    crate::panels::length_drag(&mut mm, app.units())
                                        .range(-10.0..=10.0)
                                        .prefix(format!("{} ", app.i18n.t(key))),
                                );
                                *v = mm as f32;
                                r.labelled_by(l);
                            }
                        });
                        ui.checkbox(&mut form.mirror, app.i18n.t("print-mirror"));
                        ui.checkbox(&mut form.rotate_180, app.i18n.t("print-rotate-180"));
                    });
                // Check summary and the labels count.
                let total = doc.as_ref().map_or(0, |d| total_labels(d, &form));
                let issues = app
                    .ui
                    .check
                    .as_ref()
                    .and_then(|c| c["issues"].as_array().map(Vec::len));
                let (fill, icon, color, text) = match issues {
                    Some(0) => (
                        theme::GREEN_TINT,
                        "check",
                        theme::GREEN,
                        app.i18n.t("print-check-ok"),
                    ),
                    Some(n) => (
                        theme::AMBER_TINT,
                        "warn",
                        theme::AMBER,
                        app.i18n
                            .t_args("check-count", &[("count", Arg::Num(n as f64))]),
                    ),
                    None => (theme::BG, "info", theme::MUTED, app.i18n.t("check-not-run")),
                };
                egui::Frame::new()
                    .fill(fill)
                    .corner_radius(CornerRadius::same(theme::RADIUS))
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            icons::show(ui, icon, 15.0, color);
                            ui.label(RichText::new(text).size(12.5).color(theme::TEXT2));
                            theme::trailing(ui, |ui| {
                                ui.label(
                                    RichText::new(app.i18n.t_args(
                                        "print-labels",
                                        &[("count", Arg::Num(total as f64))],
                                    ))
                                    .font(theme::bold(13.0)),
                                );
                            });
                        });
                    });
            },
        );
    });
    app.ui.print = form.clone();
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(4.0);
    let total = doc.as_ref().map_or(0, |d| total_labels(d, &form));
    ui.horizontal(|ui| {
        icons::show(ui, "sparkle", 14.0, theme::MUTED);
        let note = if app.ui.allow_agent_print {
            "print-agents-allowed"
        } else {
            "print-agents-note"
        };
        ui.label(
            RichText::new(app.i18n.t(note))
                .size(12.0)
                .color(theme::MUTED),
        );
        theme::trailing(ui, |ui| {
            let label = app
                .i18n
                .t_args("action-print-n", &[("count", Arg::Num(total as f64))]);
            if icons::text_button(
                ui,
                "printer",
                &label,
                Some(theme::PRIMARY),
                Color32::WHITE,
                None,
            )
            .clicked()
                && submit(app, &form, &connected)
            {
                close = true;
            }
            if icons::text_button(
                ui,
                "close",
                &app.i18n.t("action-cancel"),
                None,
                theme::TEXT,
                None,
            )
            .clicked()
            {
                close = true;
            }
        });
    });
    close
}

/// Queue the print job; true when it was queued.
fn submit(
    app: &mut LabelyApp,
    form: &PrintForm,
    connected: &[labely_core::services::DeviceInfo],
) -> bool {
    let connected = connected.iter().any(|d| d.address == form.address);
    if !(connected || crate::menu::connect(app, &form.address)) {
        return false;
    }
    let mut p = json!({"address": form.address, "copies": form.copies, "wait": false});
    if form.density > 0 {
        p["density"] = json!(form.density);
    }
    if app.session.doc(None).is_some_and(|d| d.data.is_some()) {
        if !form.records.trim().is_empty() {
            p["records"] = json!(form.records.trim());
        }
        if !form.copies_column.is_empty() {
            p["copies_column"] = json!(form.copies_column);
        }
    } else if form.count > 1 {
        p["count"] = json!(form.count);
    }
    if form.offset_x_mm != 0.0 {
        p["offset_x_mm"] = json!(form.offset_x_mm);
    }
    if form.offset_y_mm != 0.0 {
        p["offset_y_mm"] = json!(form.offset_y_mm);
    }
    if form.mirror {
        p["mirror"] = json!(true);
    }
    if form.rotate_180 {
        p["rotate_180"] = json!(true);
    }
    if app.run_reporting("print.submit", p).ok {
        app.set_status(app.i18n.t("status-queued"), false);
        app.ui.show_printers = true;
        true
    } else {
        false
    }
}

/// Save, discard or keep a document with unsaved changes.
fn confirm_close(app: &mut LabelyApp, ui: &mut Ui, doc_id: &str) -> bool {
    let name = app
        .session
        .doc(Some(doc_id))
        .map_or_else(String::new, |d| d.name.clone());
    ui.heading(app.i18n.t("close-title"));
    ui.label(
        app.i18n
            .t_args("close-unsaved", &[("name", Arg::Str(&name))]),
    );
    ui.add_space(8.0);
    let mut done = false;
    ui.horizontal(|ui| {
        if ui.button(app.i18n.t("action-save")).clicked() {
            app.activate(doc_id);
            crate::menu::save(app);
            if !app.session.is_dirty(Some(doc_id)) {
                app.close_doc(doc_id);
            }
            done = true;
        }
        if ui.button(app.i18n.t("action-discard")).clicked() {
            app.close_doc(doc_id);
            done = true;
        }
        if ui.button(app.i18n.t("action-cancel")).clicked() {
            done = true;
        }
    });
    done
}

fn about(app: &mut LabelyApp, ui: &mut Ui) -> bool {
    ui.heading("Labely");
    ui.label(format!(
        "{} {}",
        app.i18n.t("about-version"),
        env!("CARGO_PKG_VERSION")
    ));
    ui.label(app.i18n.t("about-text"));
    if let Some(h) = &app.mcp {
        ui.label(format!("MCP: {}", h.url));
        ui.label(format!("{}", h.conn_path.display()));
    }
    ui.button(app.i18n.t("action-close")).clicked()
}

fn print_preview(app: &LabelyApp, ui: &mut Ui, form: &PrintForm, area: egui::Rect) {
    let Some(r) = &app.raster else {
        return;
    };
    let (w, h) = (r.size[0] as f32, r.size[1] as f32);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = (area.width() / w).min(area.height() / h).min(2.0);
    // Mirroring flips x; a 180° turn flips both axes.
    let flip_x = form.mirror != form.rotate_180;
    let flip_y = form.rotate_180;
    let (u0, u1) = if flip_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let (v0, v1) = if flip_y { (1.0, 0.0) } else { (0.0, 1.0) };
    let rect = egui::Rect::from_center_size(area.center(), vec2(w * scale, h * scale));
    ui.painter().add(
        egui::Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(28),
        }
        .as_shape(rect, CornerRadius::same(6)),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), Color32::WHITE);
    egui::Image::new((r.texture.id(), rect.size()))
        .uv(egui::Rect::from_min_max(
            egui::pos2(u0, v0),
            egui::pos2(u1, v1),
        ))
        .paint_at(ui, rect);
}

#[cfg(test)]
mod tests {
    use super::parse_records;

    #[test]
    fn export_targets() {
        let f = super::ExportForm {
            pdf: false,
            dpi: 300,
            all: true,
            path: "/x/Tag.png".into(),
        };
        assert_eq!(f.target(None), "/x/Tag.png");
        assert_eq!(f.target(Some(7)), "/x/Tag-007.png");
        let f = super::ExportForm {
            path: "/x/Tag-{record}".into(),
            ..f
        };
        assert_eq!(f.target(Some(12)), "/x/Tag-012.png");
        let f = super::ExportForm {
            pdf: true,
            path: "/x/Tag.png".into(),
            ..f
        };
        assert_eq!(f.target(None), "/x/Tag.pdf");
    }

    #[test]
    fn record_lists() {
        assert_eq!(parse_records("", 3), vec![1, 2, 3]);
        assert_eq!(parse_records("1,3-4, 9", 5), vec![1, 3, 4]);
        assert_eq!(parse_records("x", 5), Vec::<usize>::new());
    }
}
