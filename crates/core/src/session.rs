//! `Session`: open documents, undo history, journal. `execute` is the only mutation path.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{CommandError, CommandKind, Ctx};
use crate::document::{Document, ElementId};
use crate::envelope::Envelope;
use crate::registry::Registry;
use crate::services::Services;

/// Maximum undo steps kept per document.
pub const UNDO_CAP: usize = 200;

/// Id of the batch command (it keeps its own `terse` param).
pub(crate) const BATCH_ID: &str = "command.batch";

/// One undo step: the document as it was before the step.
#[derive(Debug, Clone)]
pub(crate) struct HistoryStep {
    pub(crate) doc: Arc<Document>,
    pub(crate) label: String,
    pub(crate) coalesce: Option<String>,
}

/// An open document with its history.
#[derive(Debug, Clone)]
pub(crate) struct OpenDoc {
    pub(crate) doc: Arc<Document>,
    pub(crate) path: Option<PathBuf>,
    /// Content at the last save/open; `None` for never-saved documents.
    pub(crate) saved: Option<Arc<Document>>,
    pub(crate) undo: VecDeque<HistoryStep>,
    pub(crate) redo: Vec<HistoryStep>,
}

impl OpenDoc {
    pub(crate) fn new(doc: Document, path: Option<PathBuf>) -> Self {
        let doc = Arc::new(doc);
        let saved = path.as_ref().map(|_| doc.clone());
        Self {
            doc,
            path,
            saved,
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        match &self.saved {
            Some(s) => !(Arc::ptr_eq(s, &self.doc) || **s == *self.doc),
            None => true,
        }
    }

    pub(crate) fn push_undo(&mut self, step: HistoryStep) {
        self.undo.push_back(step);
        while self.undo.len() > UNDO_CAP {
            self.undo.pop_front();
        }
    }
}

/// Everything restorable: documents, histories, active document.
#[derive(Debug, Clone, Default)]
pub(crate) struct State {
    pub(crate) docs: BTreeMap<String, OpenDoc>,
    pub(crate) active: Option<String>,
    pub(crate) next_doc: u64,
    /// `(doc_id, key)` of the last coalescing edit.
    pub(crate) coalesce: Option<(String, String)>,
}

impl State {
    pub(crate) fn add_doc(&mut self, open: OpenDoc) -> String {
        self.next_doc += 1;
        let id = format!("doc{}", self.next_doc);
        self.docs.insert(id.clone(), open);
        self.active = Some(id.clone());
        id
    }
}

/// A journaled command invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub params: Value,
}

/// Editing session. Create with [`Session::new`], drive with [`Session::execute`].
pub struct Session {
    pub(crate) state: State,
    pub(crate) journal: Vec<JournalEntry>,
    pub(crate) batch_depth: u32,
    services: Services,
    registry: &'static Registry,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("docs", &self.state.docs.keys().collect::<Vec<_>>())
            .field("active", &self.state.active)
            .field("journal_len", &self.journal.len())
            .finish_non_exhaustive()
    }
}

impl Session {
    pub fn new(services: Services) -> Self {
        Self {
            state: State::default(),
            journal: Vec::new(),
            batch_depth: 0,
            services,
            registry: Registry::builtin(),
        }
    }

    pub fn services(&self) -> &Services {
        &self.services
    }

    pub fn registry(&self) -> &'static Registry {
        self.registry
    }

    /// Successful `journal` commands, in order.
    pub fn journal(&self) -> &[JournalEntry] {
        &self.journal
    }

    pub fn active_doc_id(&self) -> Option<&str> {
        self.state.active.as_deref()
    }

    pub fn doc_ids(&self) -> impl Iterator<Item = &str> {
        self.state.docs.keys().map(String::as_str)
    }

    /// A document by id, or the active one.
    pub fn doc(&self, doc_id: Option<&str>) -> Option<&Document> {
        let id = doc_id.or(self.state.active.as_deref())?;
        self.state.docs.get(id).map(|o| &*o.doc)
    }

    pub fn can_undo(&self, doc_id: Option<&str>) -> bool {
        self.open(doc_id).is_some_and(|o| !o.undo.is_empty())
    }

    pub fn can_redo(&self, doc_id: Option<&str>) -> bool {
        self.open(doc_id).is_some_and(|o| !o.redo.is_empty())
    }

    fn open(&self, doc_id: Option<&str>) -> Option<&OpenDoc> {
        let id = doc_id.or(self.state.active.as_deref())?;
        self.state.docs.get(id)
    }

    /// Execute command `id`. The only way to change session state.
    ///
    /// Envelope-level params, removed before the command sees them: `terse: bool` (drop
    /// `result`) and `coalesce: "<key>"` (merge consecutive same-key edits into one undo step).
    pub fn execute(&mut self, id: &str, params: Value) -> Envelope {
        let Some(entry) = self.registry.get(id).copied() else {
            return Envelope::failure(self.registry.unknown(id));
        };
        let mut params = match params {
            Value::Null => Value::Object(Map::new()),
            v @ Value::Object(_) => v,
            other => {
                return Envelope::failure(
                    CommandError::bad_params(format!(
                        "params for `{id}` must be a JSON object, got {}",
                        type_name(&other)
                    ))
                    .with_hint("pass {} for commands without params"),
                );
            }
        };
        let (terse, coalesce) = match take_envelope_params(id, &mut params) {
            Ok(v) => v,
            Err(e) => return Envelope::failure(e),
        };

        let before = self.state.clone();
        let journal_len = self.journal.len();
        let journal_params = params.clone();
        let depth = self.batch_depth;

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut ctx = Ctx::new(self);
            let r = (entry.run)(&mut ctx, params);
            (r, std::mem::take(&mut ctx.warnings))
        }));
        self.batch_depth = depth;

        match outcome {
            Err(panic) => {
                self.state = before;
                self.journal.truncate(journal_len);
                Envelope::failure(
                    CommandError::internal(format!(
                        "command `{id}` panicked: {}; state was restored",
                        panic_message(panic.as_ref())
                    ))
                    .with_hint("this is a bug in Labely; please report it"),
                )
            }
            Ok((Err(e), _)) => {
                self.state = before;
                self.journal.truncate(journal_len);
                Envelope::failure(e)
            }
            Ok((Ok(result), warnings)) => {
                let (changed, created) = diff_state(&before, &self.state);
                if entry.meta.kind == CommandKind::Edit && entry.meta.undoable {
                    self.record_history(&before, id, coalesce);
                }
                if entry.meta.journal {
                    self.journal.push(JournalEntry {
                        id: id.to_owned(),
                        params: journal_params,
                    });
                }
                let result = (!terse).then_some(result);
                Envelope::success(result, changed, created, warnings)
            }
        }
    }

    /// Push one undo step per document that `before` had and that changed.
    fn record_history(&mut self, before: &State, label: &str, coalesce: Option<String>) {
        let mut touched = None;
        for (doc_id, open) in &mut self.state.docs {
            let Some(prev) = before.docs.get(doc_id) else {
                continue;
            };
            if Arc::ptr_eq(&prev.doc, &open.doc) || *prev.doc == *open.doc {
                continue;
            }
            let merge = match (&coalesce, &before.coalesce) {
                (Some(key), Some((last_doc, last_key))) => {
                    last_doc == doc_id
                        && last_key == key
                        && open
                            .undo
                            .back()
                            .is_some_and(|s| s.coalesce.as_ref() == Some(key))
                }
                _ => false,
            };
            if !merge {
                open.push_undo(HistoryStep {
                    doc: prev.doc.clone(),
                    label: label.to_owned(),
                    coalesce: coalesce.clone(),
                });
            }
            open.redo.clear();
            touched = Some(doc_id.clone());
        }
        if let Some(doc_id) = touched {
            self.state.coalesce = coalesce.map(|k| (doc_id, k));
        }
    }

    /// After a successful atomic batch: replace the steps' history with one step per document.
    pub(crate) fn collapse_history(&mut self, before: &State, label: &str) {
        for (doc_id, open) in &mut self.state.docs {
            match before.docs.get(doc_id) {
                Some(prev) => {
                    open.undo = prev.undo.clone();
                    open.redo = prev.redo.clone();
                    if !(Arc::ptr_eq(&prev.doc, &open.doc) || *prev.doc == *open.doc) {
                        open.push_undo(HistoryStep {
                            doc: prev.doc.clone(),
                            label: label.to_owned(),
                            coalesce: None,
                        });
                        open.redo.clear();
                    }
                }
                None => {
                    open.undo.clear();
                    open.redo.clear();
                }
            }
        }
        self.state.coalesce = None;
    }
}

fn take_envelope_params(
    id: &str,
    params: &mut Value,
) -> Result<(bool, Option<String>), CommandError> {
    let Value::Object(map) = params else {
        return Ok((false, None));
    };
    let terse = if id == BATCH_ID {
        false
    } else {
        match map.remove("terse") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => b,
            Some(other) => {
                return Err(CommandError::bad_params(format!(
                    "`terse` must be a boolean, got {}",
                    type_name(&other)
                ))
                .with_param("terse"));
            }
        }
    };
    let coalesce = match map.remove("coalesce") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if !s.is_empty() => Some(s),
        Some(_) => {
            return Err(
                CommandError::bad_params("`coalesce` must be a non-empty string key")
                    .with_param("coalesce"),
            );
        }
    };
    Ok((terse, coalesce))
}

/// Element ids changed (modified/removed) and created between two states.
fn diff_state(before: &State, after: &State) -> (Vec<ElementId>, Vec<ElementId>) {
    let mut changed = BTreeSet::new();
    let mut created = BTreeSet::new();
    for (doc_id, open) in &after.docs {
        let prev = before.docs.get(doc_id);
        if let Some(prev) = prev
            && Arc::ptr_eq(&prev.doc, &open.doc)
        {
            continue;
        }
        let old: BTreeMap<ElementId, _> = prev
            .map(|p| p.doc.elements().map(|e| (e.id, e)).collect())
            .unwrap_or_default();
        let new: BTreeMap<ElementId, _> = open.doc.elements().map(|e| (e.id, e)).collect();
        for (id, e) in &new {
            match old.get(id) {
                None => {
                    created.insert(*id);
                }
                Some(o) if *o != *e => {
                    changed.insert(*id);
                }
                Some(_) => {}
            }
        }
        changed.extend(old.keys().filter(|id| !new.contains_key(id)));
        // Z-order changes leave elements equal; report the moved ones too.
        if let Some(prev) = prev {
            let a = prev
                .doc
                .elements()
                .map(|e| e.id)
                .filter(|id| new.contains_key(id));
            let b = open
                .doc
                .elements()
                .map(|e| e.id)
                .filter(|id| old.contains_key(id));
            for (x, y) in a.zip(b) {
                if x != y {
                    changed.insert(x);
                    changed.insert(y);
                }
            }
        }
    }
    (changed.into_iter().collect(), created.into_iter().collect())
}

pub(crate) fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_owned()
    }
}
