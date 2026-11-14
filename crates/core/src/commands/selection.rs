//! `selection.set`, `selection.get`: which elements the user (or agent) is working on.
//!
//! Selection is view state: it is per document, never journaled and never undone. The studio
//! shows it; agents can read it to act on "the selected element".

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::doc::DocIdParams;
use crate::command::{Command, CommandError, CommandMeta, Ctx, element_not_found};
use crate::document::ElementId;

/// How `ids` combine with the current selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    #[default]
    Replace,
    Add,
    Remove,
    Toggle,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Element ids; `[]` with mode `replace` clears the selection.
    pub ids: Vec<ElementId>,
    #[serde(default)]
    pub mode: SelectMode,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SelectionOutput {
    pub ids: Vec<ElementId>,
}

/// `selection.set`
#[derive(Debug)]
pub struct SelectionSet;

impl Command for SelectionSet {
    const ID: &'static str = "selection.set";
    type Params = SelectParams;
    type Output = SelectionOutput;

    fn meta() -> CommandMeta {
        CommandMeta::ui(
            "Select",
            "Set the selected elements of a document (default: active). mode: replace \
             (default), add, remove or toggle. Not undoable.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SelectParams) -> Result<SelectionOutput, CommandError> {
        let id = ctx.doc_id(p.doc_id.as_deref())?;
        let doc = ctx.doc(Some(&id))?;
        if let Some(missing) = p.ids.iter().find(|e| doc.element(**e).is_none()) {
            return Err(element_not_found(doc, *missing).with_param("ids"));
        }
        let open = ctx
            .session()
            .state
            .docs
            .get_mut(&id)
            .ok_or_else(CommandError::no_document)?;
        let sel = &mut open.selection;
        match p.mode {
            SelectMode::Replace => *sel = Vec::new(),
            SelectMode::Remove => sel.retain(|e| !p.ids.contains(e)),
            SelectMode::Add | SelectMode::Toggle => {}
        }
        for e in p.ids {
            let at = sel.iter().position(|s| *s == e);
            match (p.mode, at) {
                (SelectMode::Toggle, Some(i)) => {
                    sel.remove(i);
                }
                (SelectMode::Remove, _) | (_, Some(_)) => {}
                (_, None) => sel.push(e),
            }
        }
        Ok(SelectionOutput { ids: sel.clone() })
    }
}

/// `selection.get`
#[derive(Debug)]
pub struct SelectionGet;

impl Command for SelectionGet {
    const ID: &'static str = "selection.get";
    type Params = DocIdParams;
    type Output = SelectionOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Get selection",
            "Ids of the selected elements of a document (default: active), in selection order.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DocIdParams) -> Result<SelectionOutput, CommandError> {
        let id = ctx.doc_id(p.doc_id.as_deref())?;
        Ok(SelectionOutput {
            ids: ctx.session.selection(Some(&id)),
        })
    }
}
