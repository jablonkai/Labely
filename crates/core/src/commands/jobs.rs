//! `job.*`: the print queue and the print history (ADR-0019).

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandError, CommandMeta, Ctx, NoParams};
use crate::jobs::{HistoryEntry, JobInfo};
use crate::session::OpenDoc;

/// Longest `job.wait`.
pub const MAX_WAIT_MS: u64 = 600_000;

#[derive(Debug, Serialize, JsonSchema)]
pub struct JobListOutput {
    /// A paused queue starts no new job.
    pub paused: bool,
    /// Queued, running and recently finished jobs, oldest first.
    pub jobs: Vec<JobInfo>,
}

/// `job.list`
#[derive(Debug)]
pub struct JobList;

impl Command for JobList {
    const ID: &'static str = "job.list";
    type Params = NoParams;
    type Output = JobListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Print jobs",
            "List the print queue: queued, printing and recently finished jobs (oldest \
             first) with progress, plus whether the queue is paused.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _: NoParams) -> Result<JobListOutput, CommandError> {
        let q = ctx.session().jobs();
        Ok(JobListOutput {
            paused: q.is_paused(),
            jobs: q.list(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobIdParams {
    /// Job id from print.submit or job.list, e.g. `job-3`.
    pub id: String,
}

/// `job.get`
#[derive(Debug)]
pub struct JobGet;

impl Command for JobGet {
    const ID: &'static str = "job.get";
    type Params = JobIdParams;
    type Output = JobInfo;

    fn meta() -> CommandMeta {
        CommandMeta::query("Print job", "One print job with its state and progress.")
    }

    fn run(ctx: &mut Ctx<'_>, p: JobIdParams) -> Result<JobInfo, CommandError> {
        ctx.session().jobs().get(&p.id)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitParams {
    pub id: String,
    /// Longest wait in milliseconds (default 60000, max 600000).
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// `job.wait`
#[derive(Debug)]
pub struct JobWait;

impl Command for JobWait {
    const ID: &'static str = "job.wait";
    type Params = WaitParams;
    type Output = JobInfo;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Wait for print job",
            "Block until the job is done, failed or cancelled, or timeout_ms (default 60000) \
             passes; returns the job. Check `state`: it is still `queued` or `printing` after \
             a timeout.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: WaitParams) -> Result<JobInfo, CommandError> {
        let t = p.timeout_ms.unwrap_or(60_000);
        if t > MAX_WAIT_MS {
            return Err(
                CommandError::bad_params(format!("timeout_ms must be ≤ {MAX_WAIT_MS}"))
                    .with_param("timeout_ms"),
            );
        }
        ctx.session()
            .jobs()
            .wait(&p.id, Some(Duration::from_millis(t)))
    }
}

/// `job.cancel`
#[derive(Debug)]
pub struct JobCancel;

impl Command for JobCancel {
    const ID: &'static str = "job.cancel";
    type Params = JobIdParams;
    type Output = JobInfo;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Cancel print job",
            "Cancel a queued job, or stop the printing one at the next label. A printing job \
             turns `cancelled` once the printer has stopped; follow with job.wait.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: JobIdParams) -> Result<JobInfo, CommandError> {
        ctx.session().jobs().cancel(&p.id)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PausedOutput {
    pub paused: bool,
}

/// `job.pause`
#[derive(Debug)]
pub struct JobPause;

impl Command for JobPause {
    const ID: &'static str = "job.pause";
    type Params = NoParams;
    type Output = PausedOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Pause print queue",
            "Pause the print queue: the printing job finishes, queued jobs wait for job.resume.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _: NoParams) -> Result<PausedOutput, CommandError> {
        ctx.session().jobs().set_paused(true);
        Ok(PausedOutput { paused: true })
    }
}

/// `job.resume`
#[derive(Debug)]
pub struct JobResume;

impl Command for JobResume {
    const ID: &'static str = "job.resume";
    type Params = NoParams;
    type Output = PausedOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device("Resume print queue", "Resume a paused print queue.")
    }

    fn run(ctx: &mut Ctx<'_>, _: NoParams) -> Result<PausedOutput, CommandError> {
        ctx.session().jobs().set_paused(false);
        Ok(PausedOutput { paused: false })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ClearOutput {
    /// Finished jobs removed from the list.
    pub removed: usize,
}

/// `job.clear`
#[derive(Debug)]
pub struct JobClear;

impl Command for JobClear {
    const ID: &'static str = "job.clear";
    type Params = NoParams;
    type Output = ClearOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Clear finished jobs",
            "Remove finished jobs from job.list. The print history keeps them.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _: NoParams) -> Result<ClearOutput, CommandError> {
        Ok(ClearOutput {
            removed: ctx.session().jobs().clear_finished(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryParams {
    /// Most entries to return, newest first (default 50, max 1000).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Entries to skip (default 0).
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HistoryOutput {
    /// Entries in the whole history.
    pub total: usize,
    pub entries: Vec<HistoryEntry>,
}

/// `job.history`
#[derive(Debug)]
pub struct JobHistory;

impl Command for JobHistory {
    const ID: &'static str = "job.history";
    type Params = HistoryParams;
    type Output = HistoryOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Print history",
            "Printed jobs, newest first (limit default 50, max 1000; offset). Each entry with \
             a `file` can be reopened with job.history_open.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: HistoryParams) -> Result<HistoryOutput, CommandError> {
        let limit = p.limit.unwrap_or(50);
        if !(1..=1000).contains(&limit) {
            return Err(CommandError::bad_params("limit must be 1–1000").with_param("limit"));
        }
        let all = ctx.session().jobs().history()?;
        Ok(HistoryOutput {
            total: all.len(),
            entries: all
                .into_iter()
                .skip(p.offset.unwrap_or(0))
                .take(limit)
                .collect(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryDeleteParams {
    /// History entry ids from job.history.
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HistoryDeleteOutput {
    pub deleted: Vec<String>,
}

/// `job.history_delete`
#[derive(Debug)]
pub struct JobHistoryDelete;

impl Command for JobHistoryDelete {
    const ID: &'static str = "job.history_delete";
    type Params = HistoryDeleteParams;
    type Output = HistoryDeleteOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Delete history entries",
            "Delete print-history entries and their document snapshots. Cannot be undone.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: HistoryDeleteParams) -> Result<HistoryDeleteOutput, CommandError> {
        if p.ids.is_empty() {
            return Err(CommandError::bad_params("ids is empty")
                .with_param("ids")
                .with_hint("use job.history_clear to delete everything"));
        }
        Ok(HistoryDeleteOutput {
            deleted: ctx.session().jobs().history_delete(Some(&p.ids))?,
        })
    }
}

/// `job.history_clear`
#[derive(Debug)]
pub struct JobHistoryClear;

impl Command for JobHistoryClear {
    const ID: &'static str = "job.history_clear";
    type Params = NoParams;
    type Output = HistoryDeleteOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Clear print history",
            "Delete the whole print history and its document snapshots. Cannot be undone.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, _: NoParams) -> Result<HistoryDeleteOutput, CommandError> {
        Ok(HistoryDeleteOutput {
            deleted: ctx.session().jobs().history_delete(None)?,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryOpenParams {
    /// History entry id from job.history.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HistoryOpenOutput {
    pub doc_id: String,
    pub name: String,
}

/// `job.history_open`
#[derive(Debug)]
pub struct JobHistoryOpen;

impl Command for JobHistoryOpen {
    const ID: &'static str = "job.history_open";
    type Params = HistoryOpenParams;
    type Output = HistoryOpenOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Open from history",
            "Open the document as it was printed in a history entry, as a new unsaved \
             document, and make it active. Print it again with print.submit.",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: HistoryOpenParams) -> Result<HistoryOpenOutput, CommandError> {
        let doc = ctx.session().jobs().history_doc(&p.id)?;
        let name = doc.name.clone();
        let doc_id = ctx.session().state.add_doc(OpenDoc::new(doc, None));
        Ok(HistoryOpenOutput { doc_id, name })
    }
}
