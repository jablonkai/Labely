//! `history.undo`, `history.redo`.

use schemars::JsonSchema;
use serde::Serialize;

use super::doc::DocIdParams;
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::session::HistoryStep;

#[derive(Debug, Serialize, JsonSchema)]
pub struct HistoryOutput {
    /// Label of the step that was undone/redone (the command id or batch label).
    pub label: String,
    pub can_undo: bool,
    pub can_redo: bool,
}

fn step(
    ctx: &mut Ctx<'_>,
    doc_id: Option<&str>,
    undo: bool,
) -> Result<HistoryOutput, CommandError> {
    let id = ctx.doc_id(doc_id)?;
    let state = &mut ctx.session().state;
    state.coalesce = None;
    let open = state
        .docs
        .get_mut(&id)
        .ok_or_else(CommandError::no_document)?;
    let popped = if undo {
        open.undo.pop_back()
    } else {
        open.redo.pop()
    };
    let Some(s) = popped else {
        let what = if undo { "undo" } else { "redo" };
        return Err(CommandError::disabled(format!("nothing to {what} in {id}")));
    };
    let current = HistoryStep {
        doc: open.doc.clone(),
        label: s.label.clone(),
        coalesce: None,
    };
    open.doc = s.doc;
    if undo {
        open.redo.push(current);
    } else {
        open.push_undo(current);
    }
    Ok(HistoryOutput {
        label: s.label,
        can_undo: !open.undo.is_empty(),
        can_redo: !open.redo.is_empty(),
    })
}

/// `history.undo`
#[derive(Debug)]
pub struct Undo;

impl Command for Undo {
    const ID: &'static str = "history.undo";
    type Params = DocIdParams;
    type Output = HistoryOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Undo",
            "Undo the last edit step of a document (default: active).",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: DocIdParams) -> Result<HistoryOutput, CommandError> {
        step(ctx, p.doc_id.as_deref(), true)
    }
}

/// `history.redo`
#[derive(Debug)]
pub struct Redo;

impl Command for Redo {
    const ID: &'static str = "history.redo";
    type Params = DocIdParams;
    type Output = HistoryOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Redo",
            "Redo the last undone step of a document (default: active).",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: DocIdParams) -> Result<HistoryOutput, CommandError> {
        step(ctx, p.doc_id.as_deref(), false)
    }
}

#[derive(Debug, Default, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JournalParams {
    /// Skip the first `since` entries (for incremental reads). Default 0.
    #[serde(default)]
    pub since: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct JournalOutput {
    /// Number of entries recorded so far.
    pub total: usize,
    /// Entries from `since` on; replay them in order with `command.batch` or
    /// `labely run --script`.
    pub entries: Vec<JournalItem>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct JournalItem {
    pub id: String,
    pub params: serde_json::Value,
}

/// `history.journal`
#[derive(Debug)]
pub struct Journal;

impl Command for Journal {
    const ID: &'static str = "history.journal";
    type Params = JournalParams;
    type Output = JournalOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Journal",
            "The session's successful edit commands as [{id, params}], oldest first. The list \
             is a script: pass it as command.batch steps or to `labely run --script` to replay.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: JournalParams) -> Result<JournalOutput, CommandError> {
        let j = ctx.session().journal();
        Ok(JournalOutput {
            total: j.len(),
            entries: j
                .iter()
                .skip(p.since)
                .map(|e| JournalItem {
                    id: e.id.clone(),
                    params: e.params.clone(),
                })
                .collect(),
        })
    }
}
