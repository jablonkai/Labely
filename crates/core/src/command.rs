//! The `Command` trait, its metadata, errors and the execution context (ADR-0004).

use std::sync::Arc;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::document::{Document, Element, ElementId};
use crate::services::{DeviceError, Services};
use crate::session::Session;

/// What a command does; decides undo and batch behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    /// Reads state; pushes no undo step.
    Query,
    /// Changes a document; one undo step when `undoable`.
    Edit,
    /// Talks to a printer; pushes no undo step and is refused inside atomic batches.
    Device,
}

/// Static description of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommandMeta {
    /// Short human label, e.g. for menus.
    pub label: &'static str,
    /// Agent-facing description; states units and defaults.
    pub description: &'static str,
    pub kind: CommandKind,
    /// Recorded in the session journal on success.
    pub journal: bool,
    /// The session snapshots changed documents and pushes one undo step.
    pub undoable: bool,
}

impl CommandMeta {
    /// Read-only command.
    pub const fn query(label: &'static str, description: &'static str) -> Self {
        Self {
            label,
            description,
            kind: CommandKind::Query,
            journal: false,
            undoable: false,
        }
    }

    /// Document edit: journaled and undoable.
    pub const fn edit(label: &'static str, description: &'static str) -> Self {
        Self {
            label,
            description,
            kind: CommandKind::Edit,
            journal: true,
            undoable: true,
        }
    }

    /// Device command: not journaled, not undoable.
    pub const fn device(label: &'static str, description: &'static str) -> Self {
        Self {
            label,
            description,
            kind: CommandKind::Device,
            journal: false,
            undoable: false,
        }
    }

    pub const fn journal(mut self, journal: bool) -> Self {
        self.journal = journal;
        self
    }

    pub const fn undoable(mut self, undoable: bool) -> Self {
        self.undoable = undoable;
        self
    }
}

/// A named, schema-described operation. The only way to change a document.
pub trait Command {
    /// Dotted id, `area.verb[_object]`.
    const ID: &'static str;
    type Params: DeserializeOwned + JsonSchema;
    type Output: Serialize + JsonSchema;

    fn meta() -> CommandMeta;

    /// Whether the command can run now; `Err` is reported as `disabled`.
    fn enabled(_ctx: &Ctx<'_>) -> Result<(), CommandError> {
        Ok(())
    }

    fn run(ctx: &mut Ctx<'_>, p: Self::Params) -> Result<Self::Output, CommandError>;
}

/// Params for commands that take none.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// Stable error codes (prior-art §4.2 item 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnknownCommand,
    BadParams,
    Disabled,
    NoDocument,
    NotFound,
    DeviceUnavailable,
    DeviceError,
    Cancelled,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown_command",
            Self::BadParams => "bad_params",
            Self::Disabled => "disabled",
            Self::NoDocument => "no_document",
            Self::NotFound => "not_found",
            Self::DeviceUnavailable => "device_unavailable",
            Self::DeviceError => "device_error",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }
}

/// A command failure; serialised as the envelope's `error` object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    /// Offending param path, e.g. `width_mm` or `props.font_size_pt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    /// Accepted values or nearest valid ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted: Option<Vec<String>>,
    /// What to do next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            param: None,
            accepted: None,
            hint: None,
        }
    }

    pub fn bad_params(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadParams, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn disabled(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Disabled, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn no_document() -> Self {
        Self::new(ErrorCode::NoDocument, "no document is open")
            .with_hint("run doc.new or doc.open first")
    }

    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    pub fn with_accepted<I, S>(mut self, accepted: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.accepted = Some(accepted.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl From<DeviceError> for CommandError {
    fn from(e: DeviceError) -> Self {
        let message = e.to_string();
        match e {
            DeviceError::NotFound(_) | DeviceError::Unavailable(_) | DeviceError::Timeout(_) => {
                Self::new(ErrorCode::DeviceUnavailable, message)
                    .with_hint("check the printer is on and in range, then run device.scan")
            }
            DeviceError::Printer { hint, .. } => {
                let e = Self::new(ErrorCode::DeviceError, message);
                if hint.is_empty() {
                    e
                } else {
                    e.with_hint(hint)
                }
            }
            DeviceError::Other(_) => Self::new(ErrorCode::DeviceError, message),
        }
    }
}

/// Execution context handed to [`Command::run`].
pub struct Ctx<'a> {
    pub(crate) session: &'a mut Session,
    pub(crate) warnings: Vec<String>,
}

impl std::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("warnings", &self.warnings)
            .finish_non_exhaustive()
    }
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(session: &'a mut Session) -> Self {
        Self {
            session,
            warnings: Vec::new(),
        }
    }

    pub fn services(&self) -> &Services {
        self.session.services()
    }

    /// Add a warning to the envelope.
    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// Resolve an optional `doc_id` param to an open document id (default: active).
    pub fn doc_id(&self, doc_id: Option<&str>) -> Result<String, CommandError> {
        let state = &self.session.state;
        match doc_id {
            Some(id) if state.docs.contains_key(id) => Ok(id.to_owned()),
            Some(id) => Err(CommandError::not_found(format!("no open document `{id}`"))
                .with_param("doc_id")
                .with_accepted(state.docs.keys().cloned())),
            None => state.active.clone().ok_or_else(CommandError::no_document),
        }
    }

    pub fn doc(&self, doc_id: Option<&str>) -> Result<&Document, CommandError> {
        let id = self.doc_id(doc_id)?;
        self.session
            .state
            .docs
            .get(&id)
            .map(|o| &*o.doc)
            .ok_or_else(CommandError::no_document)
    }

    /// Shared handle to the document (cheap clone).
    pub fn doc_arc(&self, doc_id: Option<&str>) -> Result<Arc<Document>, CommandError> {
        let id = self.doc_id(doc_id)?;
        self.session
            .state
            .docs
            .get(&id)
            .map(|o| o.doc.clone())
            .ok_or_else(CommandError::no_document)
    }

    /// Mutable document; copies on write so undo snapshots stay intact.
    pub fn doc_mut(&mut self, doc_id: Option<&str>) -> Result<&mut Document, CommandError> {
        let id = self.doc_id(doc_id)?;
        self.session
            .state
            .docs
            .get_mut(&id)
            .map(|o| Arc::make_mut(&mut o.doc))
            .ok_or_else(CommandError::no_document)
    }

    pub(crate) fn session(&mut self) -> &mut Session {
        self.session
    }
}

/// `not_found` error for an element id, listing known ids.
pub(crate) fn element_not_found(doc: &Document, id: ElementId) -> CommandError {
    CommandError::not_found(format!("no element with id {} in this document", id.0))
        .with_param("id")
        .with_accepted(doc.elements().take(50).map(|e| e.id.0.to_string()))
        .with_hint("run doc.get to list element ids")
}

/// Look up an element or fail with `not_found`.
pub(crate) fn element(doc: &Document, id: ElementId) -> Result<&Element, CommandError> {
    doc.element(id).ok_or_else(|| element_not_found(doc, id))
}

/// Look up an element mutably or fail with `not_found`.
pub(crate) fn element_mut(doc: &mut Document, id: ElementId) -> Result<&mut Element, CommandError> {
    if doc.element(id).is_none() {
        return Err(element_not_found(doc, id));
    }
    doc.element_mut(id)
        .ok_or_else(|| CommandError::internal("element vanished"))
}
