//! Print queue and local print history (ADR-0019).
//!
//! `print.submit` renders on the caller's thread and hands the rasters to the [`PrintQueue`].
//! One worker thread sends queued jobs to their printers in order. A paused queue starts no
//! new job; the running one finishes. Cancelling a queued job drops it; cancelling the running
//! job stops it at the next page boundary. Jobs that reached a printer are recorded in the
//! history directory, with a snapshot of the document, when the frontend set one.
//!
//! A frontend can set a [`JobObserver`] to be told about every change instead of polling
//! `job.get` (ADR-0031).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{CommandError, ErrorCode};
use crate::document::Document;
use crate::file;
use crate::services::{DeviceError, DeviceHub, JobControl, PrintRequest};

/// Finished jobs kept in the in-memory list (the history keeps all of them).
pub const FINISHED_KEPT: usize = 100;

/// History index file inside the history directory.
pub const HISTORY_FILE: &str = "history.json";

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Printing,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// A queued, running or finished print job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    /// `job-<n>`, unique within the session.
    pub id: String,
    pub state: JobState,
    pub address: String,
    /// Printer model id, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Name of the printed document.
    pub doc_name: String,
    /// Distinct labels (records or count, times pages).
    pub labels: u32,
    pub copies: u16,
    pub density: u8,
    /// Pages the printer will feed (labels × copies).
    pub pages_total: u32,
    /// Pages printed so far.
    pub pages_done: u32,
    /// Local time, `YYYY-MM-DD HH:MM:SS`.
    pub submitted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Why the job failed or was cancelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
}

/// What [`PrintQueue::submit`] needs besides the rasters.
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub address: String,
    pub model: Option<String>,
    pub labels: u32,
    pub copies: u16,
    pub density: u8,
    pub pages_total: u32,
    /// Snapshot stored with the history entry.
    pub doc: Arc<Document>,
}

/// Told about every job change: queued, started, a page printed, finished or cancelled.
///
/// It runs on the thread that made the change while the queue is locked, so events arrive in
/// order. It must return quickly and must not call into the queue (hand the info to a
/// channel instead).
pub type JobObserver = Arc<dyn Fn(&JobInfo) + Send + Sync>;

struct Job {
    info: JobInfo,
    request: Option<PrintRequest>,
    control: Arc<JobControl>,
    doc: Arc<Document>,
}

#[derive(Default)]
struct State {
    jobs: VecDeque<Job>,
    paused: bool,
    next_id: u64,
    worker: bool,
    history_dir: Option<PathBuf>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    /// Serialises history-file rewrites within the process.
    history: Mutex<()>,
    observer: Mutex<Option<JobObserver>>,
}

impl Shared {
    /// Tell the observer about `job`. The caller holds the state lock.
    fn emit(&self, job: &Job) {
        let observer = lock(&self.observer).clone();
        if let Some(f) = observer {
            f(&live(job));
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// The print queue of a session.
#[derive(Clone)]
pub struct PrintQueue {
    devices: Arc<dyn DeviceHub>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for PrintQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let st = lock(&self.shared.state);
        f.debug_struct("PrintQueue")
            .field("jobs", &st.jobs.len())
            .field("paused", &st.paused)
            .finish_non_exhaustive()
    }
}

impl PrintQueue {
    pub fn new(devices: Arc<dyn DeviceHub>) -> Self {
        Self {
            devices,
            shared: Arc::new(Shared {
                state: Mutex::default(),
                changed: Condvar::new(),
                history: Mutex::new(()),
                observer: Mutex::new(None),
            }),
        }
    }

    /// Directory of the print history; `None` keeps no history.
    pub fn set_history_dir(&self, dir: Option<PathBuf>) {
        lock(&self.shared.state).history_dir = dir;
    }

    /// Set or clear the job observer.
    pub fn set_observer(&self, observer: Option<JobObserver>) {
        *lock(&self.shared.observer) = observer;
    }

    pub fn history_dir(&self) -> Option<PathBuf> {
        lock(&self.shared.state).history_dir.clone()
    }

    /// Queue a job; returns its info. Starts the worker when it is not running.
    pub fn submit(&self, spec: JobSpec, request: PrintRequest) -> Result<JobInfo, CommandError> {
        let mut st = lock(&self.shared.state);
        st.next_id += 1;
        let info = JobInfo {
            id: format!("job-{}", st.next_id),
            state: JobState::Queued,
            address: spec.address,
            model: spec.model,
            doc_name: spec.doc.name.clone(),
            labels: spec.labels,
            copies: spec.copies,
            density: spec.density,
            pages_total: spec.pages_total,
            pages_done: 0,
            submitted_at: now(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            error: None,
        };
        let shared = Arc::downgrade(&self.shared);
        let id = info.id.clone();
        let control =
            JobControl::with_progress_hook(Box::new(move |_, _| progressed(&shared, &id)));
        st.jobs.push_back(Job {
            info: info.clone(),
            request: Some(request),
            control: Arc::new(control),
            doc: spec.doc,
        });
        if !st.worker {
            let queue = self.clone();
            std::thread::Builder::new()
                .name("labely-print".into())
                .spawn(move || queue.work())
                .map_err(|e| {
                    st.jobs.pop_back();
                    CommandError::new(
                        ErrorCode::Internal,
                        format!("cannot start the print worker: {e}"),
                    )
                })?;
            st.worker = true;
        }
        if let Some(job) = st.jobs.back() {
            self.shared.emit(job);
        }
        self.shared.changed.notify_all();
        Ok(info)
    }

    /// All jobs, oldest first, with live progress.
    pub fn list(&self) -> Vec<JobInfo> {
        lock(&self.shared.state).jobs.iter().map(live).collect()
    }

    pub fn get(&self, id: &str) -> Result<JobInfo, CommandError> {
        let st = lock(&self.shared.state);
        find(&st, id).map(live)
    }

    pub fn is_paused(&self) -> bool {
        lock(&self.shared.state).paused
    }

    /// Pause or resume the queue.
    pub fn set_paused(&self, paused: bool) {
        lock(&self.shared.state).paused = paused;
        self.shared.changed.notify_all();
    }

    /// Whether a job is queued or printing.
    pub fn is_busy(&self) -> bool {
        lock(&self.shared.state)
            .jobs
            .iter()
            .any(|j| !j.info.state.is_finished())
    }

    /// Cancel a queued job, or ask the running one to stop.
    pub fn cancel(&self, id: &str) -> Result<JobInfo, CommandError> {
        let mut st = lock(&self.shared.state);
        let job = st
            .jobs
            .iter_mut()
            .find(|j| j.info.id == id)
            .ok_or_else(|| unknown_job(id))?;
        match job.info.state {
            JobState::Queued => {
                job.request = None;
                job.info.state = JobState::Cancelled;
                job.info.finished_at = Some(now());
                job.info.error = Some(CommandError::new(
                    ErrorCode::Cancelled,
                    "cancelled before it started",
                ));
            }
            JobState::Printing => job.control.cancel(),
            s => {
                return Err(CommandError::bad_params(format!(
                    "job {id} is already {}",
                    state_name(s)
                ))
                .with_param("id"));
            }
        }
        let info = live(job);
        if info.state == JobState::Cancelled {
            self.shared.emit(job);
        }
        self.shared.changed.notify_all();
        Ok(info)
    }

    /// Drop finished jobs from the list; returns how many.
    pub fn clear_finished(&self) -> usize {
        let mut st = lock(&self.shared.state);
        let before = st.jobs.len();
        st.jobs.retain(|j| !j.info.state.is_finished());
        before - st.jobs.len()
    }

    /// Block until the job finishes or `timeout` passes (`None`: no limit).
    pub fn wait(&self, id: &str, timeout: Option<Duration>) -> Result<JobInfo, CommandError> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut st = lock(&self.shared.state);
        loop {
            let job = find(&st, id)?;
            if job.info.state.is_finished() {
                return Ok(live(job));
            }
            if job.info.state == JobState::Queued && st.paused {
                let ahead_running = st.jobs.iter().any(|j| j.info.state == JobState::Printing);
                if !ahead_running && deadline.is_none() {
                    return Err(CommandError::bad_params(format!(
                        "job {id} is waiting in a paused queue"
                    ))
                    .with_hint("run job.resume, or submit with wait: false"));
                }
            }
            let step = Duration::from_millis(200);
            let wait = match deadline {
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(live(job));
                    }
                    left.min(step)
                }
                None => step,
            };
            st = self
                .shared
                .changed
                .wait_timeout(st, wait)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
        }
    }

    fn work(&self) {
        loop {
            let (address, request, control) = {
                let mut st = lock(&self.shared.state);
                loop {
                    if !st.paused
                        && let Some(job) = st
                            .jobs
                            .iter_mut()
                            .find(|j| j.info.state == JobState::Queued)
                    {
                        job.info.state = JobState::Printing;
                        job.info.started_at = Some(now());
                        let request = job.request.take();
                        let picked = (job.info.address.clone(), request, job.control.clone());
                        self.shared.emit(job);
                        self.shared.changed.notify_all();
                        break picked;
                    }
                    let waiting = st.jobs.iter().any(|j| j.info.state == JobState::Queued);
                    if !waiting {
                        st.worker = false;
                        return;
                    }
                    st = self
                        .shared
                        .changed
                        .wait(st)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            };
            let started = Instant::now();
            let result = match request {
                Some(r) => self.devices.print(&address, r, &control),
                None => Err(DeviceError::Other("job has no pages".into())),
            };
            let finished = {
                let mut st = lock(&self.shared.state);
                let Some(job) = st
                    .jobs
                    .iter_mut()
                    .find(|j| Arc::ptr_eq(&j.control, &control))
                else {
                    continue;
                };
                job.info.finished_at = Some(now());
                job.info.duration_ms = Some(started.elapsed().as_millis() as u64);
                match result {
                    Ok(report) => {
                        job.info.state = JobState::Done;
                        job.info.pages_done = report.pages_printed;
                        job.info.duration_ms = Some(report.duration_ms);
                    }
                    Err(e) => {
                        job.info.pages_done = control.progress().0;
                        job.info.state = if matches!(e, DeviceError::Cancelled) {
                            JobState::Cancelled
                        } else {
                            JobState::Failed
                        };
                        job.info.error = Some(e.into());
                    }
                }
                self.shared.emit(job);
                let out = (job.info.clone(), job.doc.clone(), st.history_dir.clone());
                trim_finished(&mut st);
                self.shared.changed.notify_all();
                out
            };
            if let (info, doc, Some(dir)) = finished
                && let Err(e) = self.record(&dir, &info, &doc)
            {
                warn_history(&e);
            }
        }
    }

    fn record(&self, dir: &Path, info: &JobInfo, doc: &Document) -> Result<(), CommandError> {
        let _guard = lock(&self.shared.history);
        std::fs::create_dir_all(dir).map_err(|e| io_error("create", dir, &e))?;
        let mut entries = read_history(dir)?;
        let base = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let id = (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|id| {
                !entries.iter().any(|e| &e.id == id) && !dir.join(format!("{id}.labely")).exists()
            })
            .unwrap_or(base);
        let file_name = format!("{id}.labely");
        let snapshot = file::save(doc, &dir.join(&file_name)).is_ok();
        entries.push(HistoryEntry {
            id,
            printed_at: info.finished_at.clone().unwrap_or_else(now),
            doc_name: info.doc_name.clone(),
            address: info.address.clone(),
            model: info.model.clone(),
            labels: info.labels,
            copies: info.copies,
            pages_printed: info.pages_done,
            state: info.state,
            error: info.error.as_ref().map(|e| e.message.clone()),
            file: snapshot.then_some(file_name),
        });
        write_history(dir, &entries)
    }

    /// History entries, newest first.
    pub fn history(&self) -> Result<Vec<HistoryEntry>, CommandError> {
        let dir = self.require_history()?;
        let _guard = lock(&self.shared.history);
        let mut e = read_history(&dir)?;
        e.reverse();
        Ok(e)
    }

    /// Delete history entries (all when `ids` is `None`); returns the deleted ids.
    pub fn history_delete(&self, ids: Option<&[String]>) -> Result<Vec<String>, CommandError> {
        let dir = self.require_history()?;
        let _guard = lock(&self.shared.history);
        let entries = read_history(&dir)?;
        if let Some(ids) = ids
            && let Some(missing) = ids.iter().find(|id| !entries.iter().any(|e| &e.id == *id))
        {
            return Err(
                CommandError::not_found(format!("no history entry `{missing}`")).with_param("ids"),
            );
        }
        let (gone, kept): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|e| ids.is_none_or(|ids| ids.contains(&e.id)));
        for e in &gone {
            if let Some(f) = &e.file {
                let _ = std::fs::remove_file(dir.join(f));
            }
        }
        write_history(&dir, &kept)?;
        Ok(gone.into_iter().map(|e| e.id).collect())
    }

    /// The document snapshot of a history entry.
    pub fn history_doc(&self, id: &str) -> Result<Document, CommandError> {
        let dir = self.require_history()?;
        let entry = {
            let _guard = lock(&self.shared.history);
            read_history(&dir)?
        }
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| {
            CommandError::not_found(format!("no history entry `{id}`")).with_param("id")
        })?;
        let f = entry.file.ok_or_else(|| {
            CommandError::not_found(format!("history entry `{id}` has no document snapshot"))
        })?;
        file::load(&dir.join(f)).map_err(|e| {
            CommandError::new(ErrorCode::Internal, format!("history entry `{id}`: {e}"))
        })
    }

    fn require_history(&self) -> Result<PathBuf, CommandError> {
        self.history_dir().ok_or_else(|| {
            CommandError::new(ErrorCode::Disabled, "this session keeps no print history")
        })
    }
}

/// One printed job in the history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HistoryEntry {
    /// `YYYYMMDD-HHMMSS[-n]`.
    pub id: String,
    /// Local time the job finished, `YYYY-MM-DD HH:MM:SS`.
    pub printed_at: String,
    pub doc_name: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub labels: u32,
    pub copies: u16,
    pub pages_printed: u32,
    /// `done`, `failed` or `cancelled`.
    pub state: JobState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Snapshot file name in the history directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

fn read_history(dir: &Path) -> Result<Vec<HistoryEntry>, CommandError> {
    let path = dir.join(HISTORY_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            CommandError::new(
                ErrorCode::Internal,
                format!("{} is damaged: {e}", path.display()),
            )
            .with_hint("move the file away to start a new history")
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(io_error("read", &path, &e)),
    }
}

fn write_history(dir: &Path, entries: &[HistoryEntry]) -> Result<(), CommandError> {
    let path = dir.join(HISTORY_FILE);
    let tmp = dir.join(format!("{HISTORY_FILE}.tmp"));
    let json = serde_json::to_vec_pretty(entries)
        .map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
    std::fs::write(&tmp, json).map_err(|e| io_error("write", &tmp, &e))?;
    std::fs::rename(&tmp, &path).map_err(|e| io_error("write", &path, &e))
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> CommandError {
    CommandError::new(
        ErrorCode::Internal,
        format!("cannot {what} {}: {e}", path.display()),
    )
}

/// The history is best effort: a failed write must not fail the print.
fn warn_history(e: &CommandError) {
    eprintln!("labely: print history not updated: {}", e.message);
}

/// Progress hook of a job's control: tell the observer about the new page count.
fn progressed(shared: &Weak<Shared>, id: &str) {
    let Some(shared) = shared.upgrade() else {
        return;
    };
    let st = lock(&shared.state);
    if let Some(job) = st.jobs.iter().find(|j| j.info.id == id)
        && job.info.state == JobState::Printing
    {
        shared.emit(job);
    }
}

fn live(job: &Job) -> JobInfo {
    let mut info = job.info.clone();
    if info.state == JobState::Printing {
        info.pages_done = job.control.progress().0;
    }
    info
}

fn find<'a>(st: &'a State, id: &str) -> Result<&'a Job, CommandError> {
    st.jobs
        .iter()
        .find(|j| j.info.id == id)
        .ok_or_else(|| unknown_job(id))
}

fn unknown_job(id: &str) -> CommandError {
    CommandError::not_found(format!("no job `{id}`"))
        .with_param("id")
        .with_hint("run job.list for the current jobs")
}

fn state_name(s: JobState) -> &'static str {
    match s {
        JobState::Queued => "queued",
        JobState::Printing => "printing",
        JobState::Done => "done",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

fn trim_finished(st: &mut State) {
    let finished = st
        .jobs
        .iter()
        .filter(|j| j.info.state.is_finished())
        .count();
    let mut extra = finished.saturating_sub(FINISHED_KEPT);
    st.jobs.retain(|j| {
        if extra > 0 && j.info.state.is_finished() {
            extra -= 1;
            false
        } else {
            true
        }
    });
}
