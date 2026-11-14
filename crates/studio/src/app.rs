//! `LabelyApp`: session, view state and the frame loop. Every edit goes through
//! [`LabelyApp::run`], which calls `Session::execute` (ADR-0004).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Context, Ui};
use labely_core::{Document, ElementId, Envelope, Policy, Session};
use labely_i18n::{Arg, I18n, Lang};
use serde_json::{Value, json};

use crate::mcp_host::{McpCall, McpHost};
use crate::{canvas, dialogs, menu, panels};

/// Spreadsheet-style column name: 0 → A, 26 → AA.
pub fn column_name(i: usize) -> String {
    labely_core::commands::data::column_letter(i)
}

/// Start-up options.
#[derive(Debug, Clone)]
pub struct StudioOptions {
    /// Start the MCP server (Streamable HTTP on loopback).
    pub mcp: bool,
    pub mcp_port: u16,
    /// Directories agents may read and write; default: the home directory.
    pub agent_roots: Vec<PathBuf>,
    /// Document to open at start.
    pub open: Option<PathBuf>,
    pub lang: Lang,
    /// Show native file dialogs (off in tests).
    pub native_dialogs: bool,
    /// MCP connection file; default `<app-data>/mcp-connection.json`.
    pub connection_file: Option<PathBuf>,
}

impl Default for StudioOptions {
    fn default() -> Self {
        Self {
            mcp: true,
            mcp_port: labely_mcp::connection::DEFAULT_PORT,
            agent_roots: Vec::new(),
            open: None,
            lang: Lang::detect(),
            native_dialogs: true,
            connection_file: None,
        }
    }
}

/// A canvas drag in progress; one coalesced undo step.
#[derive(Debug, Clone)]
pub enum Gesture {
    Move {
        start: egui::Pos2,
        originals: Vec<(ElementId, f64, f64)>,
        /// Union of the moving elements' bounding boxes at the start, mm.
        bounds: (f64, f64, f64, f64),
        key: String,
    },
    Resize {
        id: ElementId,
        handle: (i8, i8),
        orig: canvas::Frame,
        key: String,
    },
    Rotate {
        id: ElementId,
        center: egui::Pos2,
        start_angle: f32,
        orig_deg: f64,
        key: String,
    },
}

/// A status line message.
#[derive(Debug, Clone)]
pub struct Status {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

/// Which modal is open.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    NewDoc(dialogs::NewDocForm),
    Print,
    About,
}

/// View state; never part of a document.
#[derive(Debug)]
pub struct UiState {
    /// Screen points per mm.
    pub zoom: f32,
    pub fit_pending: bool,
    pub show_layers: bool,
    pub show_props: bool,
    pub show_check: bool,
    pub show_printers: bool,
    pub show_preview: bool,
    pub show_data: bool,
    /// Snap moved elements to the label, other elements and the grid.
    pub snap: bool,
    pub show_grid: bool,
    /// Grid spacing, mm.
    pub grid_mm: f64,
    /// Label colour shown on the canvas (not printed).
    pub label_color: [u8; 3],
    /// Snap guides of the current drag, mm: (vertical x lines, horizontal y lines).
    pub guides: (Vec<f64>, Vec<f64>),
    /// Focused table cell (row, column) in the properties panel.
    pub table_cell: (usize, usize),
    /// Data record shown on the canvas, 1-based.
    pub record: usize,
    pub gesture: Option<Gesture>,
    pub gesture_seq: u64,
    pub dialog: Option<Dialog>,
    pub status: Option<Status>,
    /// Last `doc.check` result.
    pub check: Option<Value>,
    pub print: dialogs::PrintForm,
    /// Last `device.scan` result.
    pub scanned: Vec<Value>,
    pub allow_agent_print: bool,
    /// Recent agent commands, newest last.
    pub agent_log: VecDeque<String>,
    /// Canvas mapping of the last frame (for tests and tools).
    pub last_view: Option<crate::canvas::View>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            zoom: 8.0,
            fit_pending: true,
            show_layers: true,
            show_props: true,
            show_check: false,
            show_printers: false,
            show_preview: false,
            show_data: false,
            table_cell: (0, 0),
            snap: true,
            show_grid: false,
            grid_mm: 1.0,
            label_color: [255, 255, 255],
            guides: (Vec::new(), Vec::new()),
            record: 1,
            gesture: None,
            gesture_seq: 0,
            dialog: None,
            status: None,
            check: None,
            print: dialogs::PrintForm::default(),
            scanned: Vec::new(),
            allow_agent_print: false,
            agent_log: VecDeque::new(),
            last_view: None,
        }
    }
}

/// Cached 1-bit raster of the active document.
pub struct Raster {
    pub doc: Arc<Document>,
    pub dpi: u32,
    /// Binding values it was rendered with.
    pub values: std::collections::BTreeMap<String, String>,
    pub texture: egui::TextureHandle,
    pub size: [usize; 2],
}

impl std::fmt::Debug for Raster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Raster")
            .field("dpi", &self.dpi)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// The studio application.
pub struct LabelyApp {
    pub session: Session,
    pub ui: UiState,
    pub i18n: I18n,
    pub opts: StudioOptions,
    pub(crate) raster: Option<Raster>,
    pub(crate) mcp: Option<McpHost>,
    pending: VecDeque<McpCall>,
    agent_roots: Vec<PathBuf>,
}

impl std::fmt::Debug for LabelyApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelyApp").finish_non_exhaustive()
    }
}

impl LabelyApp {
    /// Build the app; `ctx` is needed to start the MCP server (it wakes the UI).
    pub fn new(ctx: Option<&Context>, opts: StudioOptions) -> Self {
        Self::with_session(ctx, Session::new(labely_printers::default_services()), opts)
    }

    pub fn with_session(ctx: Option<&Context>, session: Session, opts: StudioOptions) -> Self {
        let agent_roots = if opts.agent_roots.is_empty() {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .into_iter()
                .collect()
        } else {
            opts.agent_roots.clone()
        };
        let mut app = Self {
            session,
            ui: UiState::default(),
            i18n: I18n::new(opts.lang),
            raster: None,
            mcp: None,
            pending: VecDeque::new(),
            agent_roots,
            opts,
        };
        if let Some(path) = app.opts.open.clone() {
            app.run_reporting("doc.open", json!({ "path": path }));
        }
        if app.opts.mcp
            && let Some(ctx) = ctx
        {
            app.start_mcp(ctx);
        }
        app
    }

    /// Start the MCP server (no-op when running).
    pub fn start_mcp(&mut self, ctx: &Context) {
        if self.mcp.is_some() {
            return;
        }
        let path = self
            .opts
            .connection_file
            .clone()
            .unwrap_or_else(labely_mcp::connection::default_path);
        match McpHost::start(ctx.clone(), self.opts.mcp_port, path) {
            Ok(h) => self.mcp = Some(h),
            Err(e) => {
                let text = self.i18n.t_args("mcp-failed", &[("error", Arg::Str(&e))]);
                self.set_status(text, true);
            }
        }
    }

    /// The MCP endpoint, when the server runs.
    pub fn mcp_url(&self) -> Option<&str> {
        self.mcp.as_ref().map(|h| h.url.as_str())
    }

    /// Run a command as the user. The single mutation path of the studio.
    pub fn run(&mut self, id: &str, params: Value) -> Envelope {
        self.session.execute(id, params)
    }

    /// Run and put failures in the status line.
    pub fn run_reporting(&mut self, id: &str, params: Value) -> Envelope {
        let env = self.run(id, params);
        if let Some(e) = &env.error {
            let text = format!("{}: {}", self.i18n.error_title(e.code.as_str()), e.message);
            self.set_status(text, true);
        }
        env
    }

    /// Run a command for an agent: restricted file roots, printing only when allowed.
    pub fn run_as_agent(&mut self, id: &str, params: Value) -> Envelope {
        let user = self.session.policy().clone();
        let mut agent = Policy::restricted(self.agent_roots.clone());
        agent.allow_print = self.ui.allow_agent_print;
        self.session.set_policy(agent);
        let env = self.session.execute(id, params);
        self.session.set_policy(user);
        let mark = if env.ok { "✔" } else { "✖" };
        self.ui.agent_log.push_back(format!("{mark} {id}"));
        while self.ui.agent_log.len() > 50 {
            self.ui.agent_log.pop_front();
        }
        if id.starts_with("doc.") && env.ok {
            self.ui.fit_pending = true;
        }
        env
    }

    pub fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.ui.status = Some(Status {
            text: text.into(),
            error,
            at: Instant::now(),
        });
    }

    pub fn selection(&self) -> Vec<ElementId> {
        self.session.selection(None)
    }

    /// Select `ids`, widened to whole groups.
    pub fn select(&mut self, ids: &[ElementId]) {
        let Some(doc) = self.session.doc(None) else {
            return;
        };
        let ids = labely_core::commands::arrange::expand_groups(doc, ids);
        self.run("selection.set", json!({ "ids": ids }));
    }

    /// Next coalescing key for a gesture or a property edit session.
    pub fn next_key(&mut self, what: &str) -> String {
        self.ui.gesture_seq += 1;
        format!("{what}-{}", self.ui.gesture_seq)
    }

    /// Answer agent calls, unless a gesture or dialog is in progress (then they wait).
    fn serve_agents(&mut self, ctx: &Context) {
        let Some(host) = &self.mcp else { return };
        while let Ok(call) = host.inbox.try_recv() {
            self.pending.push_back(call);
        }
        if self.pending.is_empty() {
            return;
        }
        if self.ui.gesture.is_some() || self.ui.dialog.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        while let Some(call) = self.pending.pop_front() {
            let env = if call.expired() {
                continue;
            } else {
                self.run_as_agent(&call.id, call.params.clone())
            };
            call.reply(env);
        }
    }

    /// Lay out the whole window.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.serve_agents(&ctx);
        menu::shortcuts(self, &ctx);
        egui::Panel::top("menu").show(ui, |ui| menu::menu_bar(self, ui));
        egui::Panel::bottom("status").show(ui, |ui| panels::status_bar(self, ui));
        if self.session.active_doc_id().is_some() {
            if self.ui.show_layers {
                egui::Panel::left("layers")
                    .resizable(true)
                    .default_size(200.0)
                    .show(ui, |ui| panels::layers(self, ui));
            }
            if self.ui.show_props {
                egui::Panel::right("props")
                    .resizable(true)
                    .default_size(260.0)
                    .show(ui, |ui| panels::properties(self, ui));
            }
            if self.ui.show_check || self.ui.show_printers || self.ui.show_data {
                egui::Panel::bottom("tools")
                    .resizable(true)
                    .default_size(160.0)
                    .show(ui, |ui| panels::tools(self, ui));
            }
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.session.active_doc_id().is_some() {
                canvas::canvas(self, ui);
            } else {
                panels::welcome(self, ui);
            }
        });
        if self.ui.show_preview {
            panels::preview_window(self, &ctx);
        }
        dialogs::show(self, &ctx);
    }

    /// Title for the window: document name and dirty mark.
    pub fn title(&self) -> String {
        match self.session.doc(None) {
            Some(d) => {
                let dirty = if self.session.is_dirty(None) {
                    " •"
                } else {
                    ""
                };
                format!("{}{dirty} — Labely", d.name)
            }
            None => "Labely".to_owned(),
        }
    }
}

impl eframe::App for LabelyApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let title = self.title();
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.show(ui);
    }
}
