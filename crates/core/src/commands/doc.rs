//! `doc.*` commands (except `doc.check`, in `render`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::check_mm;
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{Document, LabelSpec, MediaType};
use crate::file::{self, FileError};
use crate::session::OpenDoc;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewParams {
    /// Document name (default `Untitled`).
    #[serde(default)]
    pub name: Option<String>,
    /// Label width across the printhead, mm.
    pub width_mm: f64,
    /// Label length along the feed, mm.
    pub height_mm: f64,
    #[serde(default)]
    pub corner_radius_mm: Option<f64>,
    /// Label stock (default `gap`).
    #[serde(default)]
    pub media: Option<MediaType>,
    /// Printer model id, e.g. `niimbot.b21_pro`; sets the preview/check dpi.
    #[serde(default)]
    pub printer_model: Option<String>,
    /// Print density; the model default is used when absent.
    #[serde(default)]
    pub density: Option<u8>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DocIdOutput {
    /// Id of the document, usable as `doc_id` in other commands.
    pub doc_id: String,
}

pub(crate) fn validate_label(l: &LabelSpec) -> Result<(), CommandError> {
    check_mm("width_mm", l.width_mm, true)?;
    check_mm("height_mm", l.height_mm, true)?;
    check_mm("corner_radius_mm", l.corner_radius_mm, false)?;
    if l.corner_radius_mm < 0.0 {
        return Err(CommandError::bad_params("`corner_radius_mm` must be >= 0")
            .with_param("corner_radius_mm"));
    }
    Ok(())
}

/// `doc.new`
#[derive(Debug)]
pub struct DocNew;

impl Command for DocNew {
    const ID: &'static str = "doc.new";
    type Params = NewParams;
    type Output = DocIdOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "New document",
            "Create an empty label document and make it active. Sizes in mm (width across the \
             printhead, height along the feed). Returns {doc_id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: NewParams) -> Result<DocIdOutput, CommandError> {
        let label = LabelSpec {
            width_mm: p.width_mm,
            height_mm: p.height_mm,
            corner_radius_mm: p.corner_radius_mm.unwrap_or(0.0),
            media: p.media.unwrap_or_default(),
            printer_model: p.printer_model.filter(|s| !s.is_empty()),
            density: p.density,
        };
        validate_label(&label)?;
        let doc = Document::new(p.name.unwrap_or_else(|| "Untitled".to_owned()), label);
        let doc_id = ctx.session().state.add_doc(OpenDoc::new(doc, None));
        Ok(DocIdOutput { doc_id })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenParams {
    /// Path of a `.labely` file.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenOutput {
    pub doc_id: String,
    pub name: String,
}

pub(crate) fn file_error(e: FileError) -> CommandError {
    match e {
        FileError::Read { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            CommandError::not_found(e.to_string()).with_param("path")
        }
        FileError::Write { .. } => CommandError::internal(e.to_string())
            .with_param("path")
            .with_hint("check the directory exists and is writable"),
        _ => CommandError::bad_params(e.to_string()).with_param("path"),
    }
}

/// `doc.open`
#[derive(Debug)]
pub struct DocOpen;

impl Command for DocOpen {
    const ID: &'static str = "doc.open";
    type Params = OpenParams;
    type Output = OpenOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Open document",
            "Open a `.labely` file as a new document and make it active. Returns {doc_id, name}.",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: OpenParams) -> Result<OpenOutput, CommandError> {
        let path = ctx.check_path("path", &p.path, false)?;
        let doc = file::load(&path).map_err(file_error)?;
        let name = doc.name.clone();
        let doc_id = ctx.session().state.add_doc(OpenDoc::new(doc, Some(path)));
        Ok(OpenOutput { doc_id, name })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Target path; defaults to where the document was opened or last saved.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SaveOutput {
    pub path: String,
    pub bytes: usize,
}

/// `doc.save`
#[derive(Debug)]
pub struct DocSave;

impl Command for DocSave {
    const ID: &'static str = "doc.save";
    type Params = SaveParams;
    type Output = SaveOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Save document",
            "Write the document as pretty `.labely` JSON. `path` defaults to the file it was \
             opened from or last saved to.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SaveParams) -> Result<SaveOutput, CommandError> {
        let id = ctx.doc_id(p.doc_id.as_deref())?;
        let stored = ctx.session.state.docs.get(&id).and_then(|o| o.path.clone());
        let path = match (p.path, stored) {
            (Some(p), _) => ctx.check_path("path", &p, true)?,
            (None, Some(p)) => ctx.check_path("path", &p.to_string_lossy(), true)?,
            (None, None) => {
                return Err(
                    CommandError::bad_params("this document has never been saved")
                        .with_param("path")
                        .with_hint("pass path, e.g. \"label.labely\""),
                );
            }
        };
        let open = ctx
            .session()
            .state
            .docs
            .get_mut(&id)
            .ok_or_else(CommandError::no_document)?;
        let bytes = file::save(&open.doc, &path).map_err(file_error)?;
        open.saved = Some(open.doc.clone());
        open.path = Some(path.clone());
        Ok(SaveOutput {
            path: path.display().to_string(),
            bytes,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocIdParams {
    /// Document id; defaults to the active document.
    #[serde(default)]
    pub doc_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseOutput {
    pub closed: String,
    /// The new active document, if any remain open.
    pub active: Option<String>,
}

/// `doc.close`
#[derive(Debug)]
pub struct DocClose;

impl Command for DocClose {
    const ID: &'static str = "doc.close";
    type Params = DocIdParams;
    type Output = CloseOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Close document",
            "Close a document (default: active) and drop its history. Unsaved changes are lost; \
             a warning says so.",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: DocIdParams) -> Result<CloseOutput, CommandError> {
        let id = ctx.doc_id(p.doc_id.as_deref())?;
        let state = &mut ctx.session().state;
        let open = state
            .docs
            .remove(&id)
            .ok_or_else(CommandError::no_document)?;
        if state.active.as_deref() == Some(id.as_str()) {
            state.active = state.docs.keys().next_back().cloned();
        }
        if state.coalesce.as_ref().is_some_and(|(d, _)| *d == id) {
            state.coalesce = None;
        }
        let active = state.active.clone();
        if open.dirty() && (open.saved.is_some() || open.doc.elements().next().is_some()) {
            ctx.warn(format!("{id} was closed with unsaved changes"));
        }
        Ok(CloseOutput { closed: id, active })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetOutput {
    pub doc_id: String,
    /// File the document was opened from or last saved to.
    pub path: Option<String>,
    /// Changed since the last open/save (always true for never-saved documents).
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// The full `.labely` document. Units are mm.
    pub document: Document,
}

/// `doc.get`
#[derive(Debug)]
pub struct DocGet;

impl Command for DocGet {
    const ID: &'static str = "doc.get";
    type Params = DocIdParams;
    type Output = GetOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Get document",
            "Return the full document JSON (units mm, elements with ids) plus doc_id, path, \
             dirty and undo/redo availability.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DocIdParams) -> Result<GetOutput, CommandError> {
        let id = ctx.doc_id(p.doc_id.as_deref())?;
        let open = ctx
            .session()
            .state
            .docs
            .get(&id)
            .ok_or_else(CommandError::no_document)?;
        Ok(GetOutput {
            doc_id: id,
            path: open.path.as_ref().map(|p| p.display().to_string()),
            dirty: open.dirty(),
            can_undo: !open.undo.is_empty(),
            can_redo: !open.redo.is_empty(),
            document: (*open.doc).clone(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetLabelParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Document name.
    #[serde(default)]
    pub name: Option<String>,
    /// Label width across the printhead, mm.
    #[serde(default)]
    pub width_mm: Option<f64>,
    /// Label length along the feed, mm.
    #[serde(default)]
    pub height_mm: Option<f64>,
    #[serde(default)]
    pub corner_radius_mm: Option<f64>,
    #[serde(default)]
    pub media: Option<MediaType>,
    /// Printer model id; empty string clears it.
    #[serde(default)]
    pub printer_model: Option<String>,
    /// Print density; 0 clears it (model default).
    #[serde(default)]
    pub density: Option<u8>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetLabelOutput {
    pub label: LabelSpec,
}

/// `doc.set_label`
#[derive(Debug)]
pub struct DocSetLabel;

impl Command for DocSetLabel {
    const ID: &'static str = "doc.set_label";
    type Params = SetLabelParams;
    type Output = SetLabelOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Label settings",
            "Change label stock settings; only the given fields change. Sizes in mm. \
             printer_model \"\" and density 0 clear those fields.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SetLabelParams) -> Result<SetLabelOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let l = &mut doc.label;
        if let Some(v) = p.width_mm {
            l.width_mm = v;
        }
        if let Some(v) = p.height_mm {
            l.height_mm = v;
        }
        if let Some(v) = p.corner_radius_mm {
            l.corner_radius_mm = v;
        }
        if let Some(v) = p.media {
            l.media = v;
        }
        if let Some(v) = p.printer_model {
            l.printer_model = (!v.is_empty()).then_some(v);
        }
        if let Some(v) = p.density {
            l.density = (v != 0).then_some(v);
        }
        validate_label(l)?;
        let label = l.clone();
        if let Some(name) = p.name {
            doc.name = name;
        }
        Ok(SetLabelOutput { label })
    }
}

/// Numeric suffix of a `doc{n}` id, for listing in open order.
fn doc_number(id: &str) -> u64 {
    id.strip_prefix("doc")
        .and_then(|n| n.parse().ok())
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenDocInfo {
    pub doc_id: String,
    pub name: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub active: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListOutput {
    /// Open documents in the order they were opened.
    pub docs: Vec<OpenDocInfo>,
}

/// `doc.list`
#[derive(Debug)]
pub struct DocList;

impl Command for DocList {
    const ID: &'static str = "doc.list";
    type Params = ListParams;
    type Output = ListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Open documents",
            "List the open documents in the order they were opened: doc_id, name, path, \
             dirty and which one is active.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _p: ListParams) -> Result<ListOutput, CommandError> {
        let state = &ctx.session().state;
        let mut docs: Vec<OpenDocInfo> = state
            .docs
            .iter()
            .map(|(id, open)| OpenDocInfo {
                doc_id: id.clone(),
                name: open.doc.name.clone(),
                path: open.path.as_ref().map(|p| p.display().to_string()),
                dirty: open.dirty(),
                active: state.active.as_deref() == Some(id.as_str()),
            })
            .collect();
        docs.sort_by_key(|d| doc_number(&d.doc_id));
        Ok(ListOutput { docs })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivateParams {
    pub doc_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ActivateOutput {
    pub active: String,
}

/// `doc.activate`
#[derive(Debug)]
pub struct DocActivate;

impl Command for DocActivate {
    const ID: &'static str = "doc.activate";
    type Params = ActivateParams;
    type Output = ActivateOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Switch document",
            "Make an open document the active one; commands without doc_id then act on it.",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: ActivateParams) -> Result<ActivateOutput, CommandError> {
        let id = ctx.doc_id(Some(&p.doc_id))?;
        let state = &mut ctx.session().state;
        if state.active.as_deref() != Some(id.as_str()) {
            state.active = Some(id.clone());
            state.coalesce = None;
        }
        Ok(ActivateOutput { active: id })
    }
}

/// Turn direction for `doc.rotate_label`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Turn {
    /// A quarter turn clockwise.
    #[default]
    Cw,
    /// A quarter turn anticlockwise.
    Ccw,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RotateLabelParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Quarter-turn direction (default `cw`).
    #[serde(default)]
    pub direction: Option<Turn>,
    /// Turn the elements with the paper (default true). With false only the label size swaps.
    #[serde(default)]
    pub rotate_content: Option<bool>,
}

/// `doc.rotate_label`
#[derive(Debug)]
pub struct DocRotateLabel;

impl Command for DocRotateLabel {
    const ID: &'static str = "doc.rotate_label";
    type Params = RotateLabelParams;
    type Output = SetLabelOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Rotate label paper",
            "Turn the label a quarter turn (direction cw or ccw): width and height swap. With \
             rotate_content (default true) every element turns with it about the label, so the \
             design keeps its look; with false the elements stay where they are.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: RotateLabelParams) -> Result<SetLabelOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let (w, h) = (doc.label.width_mm, doc.label.height_mm);
        let cw = p.direction.unwrap_or_default() == Turn::Cw;
        if p.rotate_content.unwrap_or(true) {
            for page in &mut doc.pages {
                for e in &mut page.elements {
                    let (cx, cy) = (e.x_mm + e.width_mm / 2.0, e.y_mm + e.height_mm / 2.0);
                    // y points down, so a clockwise turn maps (x, y) to (h − y, x).
                    let (nx, ny) = if cw { (h - cy, cx) } else { (cy, w - cx) };
                    e.x_mm = nx - e.width_mm / 2.0;
                    e.y_mm = ny - e.height_mm / 2.0;
                    let turn = if cw { 90.0 } else { 270.0 };
                    e.rotation_deg = (e.rotation_deg + turn).rem_euclid(360.0);
                }
            }
        }
        doc.label.width_mm = h;
        doc.label.height_mm = w;
        validate_label(&doc.label)?;
        Ok(SetLabelOutput {
            label: doc.label.clone(),
        })
    }
}
