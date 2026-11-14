//! `template.*`: the bundled starter set and the local template library (ADR-0018).
//!
//! The library is a directory set by the frontend ([`Session::set_template_dir`]): templates
//! are `.labely` files in it (the default folder) or in one level of sub-folders. Commands
//! address templates by reference, never by path:
//! - `starter:<id>`: a bundled starter template (read-only),
//! - `my:<name>` or `my:<folder>/<name>`: a library template.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::doc::{OpenOutput, file_error};
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::Document;
use crate::file;
use crate::session::{OpenDoc, Session};

/// The starter set: `(id, .labely JSON)`. Original designs, no vendor content.
pub const STARTERS: &[(&str, &str)] = &[
    (
        "name-badge",
        include_str!("../../templates/name-badge.labely"),
    ),
    (
        "price-tag",
        include_str!("../../templates/price-tag.labely"),
    ),
    (
        "asset-tag",
        include_str!("../../templates/asset-tag.labely"),
    ),
    ("address", include_str!("../../templates/address.labely")),
    ("qr-link", include_str!("../../templates/qr-link.labely")),
    (
        "storage-box",
        include_str!("../../templates/storage-box.labely"),
    ),
    ("jar", include_str!("../../templates/jar.labely")),
    (
        "cable-flag",
        include_str!("../../templates/cable-flag.labely"),
    ),
    ("wifi", include_str!("../../templates/wifi.labely")),
    ("shipping", include_str!("../../templates/shipping.labely")),
];

/// Longest template or folder name, in characters.
const MAX_NAME: usize = 80;

/// A parsed template reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRef {
    Starter(&'static str),
    Library { folder: String, name: String },
}

impl TemplateRef {
    pub fn parse(s: &str) -> Result<Self, CommandError> {
        if let Some(id) = s.strip_prefix("starter:") {
            return STARTERS
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(i, _)| Self::Starter(i))
                .ok_or_else(|| {
                    CommandError::not_found(format!("no starter template `{id}`"))
                        .with_param("ref")
                        .with_accepted(STARTERS.iter().map(|(i, _)| format!("starter:{i}")))
                });
        }
        let Some(rest) = s.strip_prefix("my:") else {
            return Err(
                CommandError::bad_params(format!("`{s}` is not a template reference"))
                    .with_param("ref")
                    .with_hint("use `starter:<id>` or `my:[<folder>/]<name>` from template.list"),
            );
        };
        let (folder, name) = rest.split_once('/').unwrap_or(("", rest));
        Ok(Self::Library {
            folder: check_folder("ref", folder)?,
            name: check_name("ref", name)?,
        })
    }

    /// The reference string.
    pub fn as_ref_string(&self) -> String {
        match self {
            Self::Starter(id) => format!("starter:{id}"),
            Self::Library { folder, name } if folder.is_empty() => format!("my:{name}"),
            Self::Library { folder, name } => format!("my:{folder}/{name}"),
        }
    }
}

/// Validate a template or folder name: no path separators or reserved characters.
fn check_name(param: &str, s: &str) -> Result<String, CommandError> {
    let t = s.trim();
    let bad = t.is_empty()
        || t.chars().count() > MAX_NAME
        || t.starts_with('.')
        || t.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        });
    if bad {
        return Err(CommandError::bad_params(format!(
            "`{s}` is not a valid name: 1–{MAX_NAME} characters, not starting with `.`, \
             without / \\ : * ? \" < > |"
        ))
        .with_param(param));
    }
    Ok(t.to_owned())
}

/// Folder name; empty is the default folder.
fn check_folder(param: &str, s: &str) -> Result<String, CommandError> {
    if s.trim().is_empty() {
        Ok(String::new())
    } else {
        check_name(param, s)
    }
}

fn library(session: &Session) -> Result<PathBuf, CommandError> {
    session
        .template_dir()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            CommandError::disabled("no template library is configured in this session")
                .with_hint("starter templates still work; the studio and CLI set a library")
        })
}

fn folder_dir(lib: &Path, folder: &str) -> PathBuf {
    if folder.is_empty() {
        lib.to_path_buf()
    } else {
        lib.join(folder)
    }
}

fn template_path(lib: &Path, folder: &str, name: &str) -> PathBuf {
    folder_dir(lib, folder).join(format!("{name}.{}", file::EXTENSION))
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> CommandError {
    CommandError::internal(format!("cannot {what} {}: {e}", path.display()))
}

/// Load a template (starter or library) as a document.
pub fn load(session: &Session, reference: &str) -> Result<Document, CommandError> {
    match TemplateRef::parse(reference)? {
        TemplateRef::Starter(id) => {
            let json = STARTERS
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, j)| *j)
                .unwrap_or_default();
            file::from_json(json)
                .map_err(|e| CommandError::internal(format!("starter `{id}` is broken: {e}")))
        }
        TemplateRef::Library { folder, name } => {
            let path = template_path(&library(session)?, &folder, &name);
            file::load(&path).map_err(|e| file_error(e).with_param("ref"))
        }
    }
}

/// Library folders, sorted; hidden directories are skipped.
fn folders(lib: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(lib)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| check_name("folder", n).is_ok())
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// Names of the templates in one folder, sorted.
fn names_in(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == file::EXTENSION)
                .then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .filter(|n| check_name("name", n).is_ok())
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// One `template.list` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TemplateInfo {
    /// Reference for the other `template.*` commands.
    #[serde(rename = "ref")]
    pub reference: String,
    pub name: String,
    /// Library folder; empty for the default folder and for starters.
    pub folder: String,
    pub starter: bool,
    pub width_mm: f64,
    pub height_mm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer_model: Option<String>,
}

fn info(reference: String, folder: &str, starter: bool, doc: &Document) -> TemplateInfo {
    TemplateInfo {
        reference,
        name: doc.name.clone(),
        folder: folder.to_owned(),
        starter,
        width_mm: doc.label.width_mm,
        height_mm: doc.label.height_mm,
        printer_model: doc.label.printer_model.clone(),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// Case-insensitive part of the name to look for.
    #[serde(default)]
    pub query: Option<String>,
    /// Only this library folder (`""` is the default folder); starters are left out.
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListOutput {
    /// Library directory, when the session has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    /// Library folders besides the default one.
    pub folders: Vec<String>,
    pub templates: Vec<TemplateInfo>,
}

/// `template.list`
#[derive(Debug)]
pub struct TemplateList;

impl Command for TemplateList {
    const ID: &'static str = "template.list";
    type Params = ListParams;
    type Output = ListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Templates",
            "List the starter templates and the local library: {library, folders, templates: \
             [{ref, name, folder, starter, width_mm, height_mm, printer_model}]}. `query` \
             filters by name; `folder` lists one library folder only. Open one with \
             template.new {ref}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ListParams) -> Result<ListOutput, CommandError> {
        let needle = p
            .query
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_default();
        let only = p
            .folder
            .as_deref()
            .map(|f| check_folder("folder", f))
            .transpose()?;
        let mut templates = Vec::new();
        if only.is_none() {
            for (id, json) in STARTERS {
                if let Ok(doc) = file::from_json(json) {
                    templates.push(info(format!("starter:{id}"), "", true, &doc));
                }
            }
        }
        let lib = ctx.session.template_dir().map(Path::to_path_buf);
        let mut folder_list = Vec::new();
        if let Some(lib) = &lib {
            folder_list = folders(lib);
            let scan: Vec<String> = match &only {
                Some(f) => vec![f.clone()],
                None => std::iter::once(String::new())
                    .chain(folder_list.iter().cloned())
                    .collect(),
            };
            for folder in scan {
                for name in names_in(&folder_dir(lib, &folder)) {
                    let r = TemplateRef::Library {
                        folder: folder.clone(),
                        name: name.clone(),
                    };
                    match file::load(&template_path(lib, &folder, &name)) {
                        Ok(mut doc) => {
                            doc.name = name;
                            templates.push(info(r.as_ref_string(), &folder, false, &doc));
                        }
                        Err(e) => ctx.warn(format!("{} skipped: {e}", r.as_ref_string())),
                    }
                }
            }
        } else if only.is_some() {
            library(ctx.session)?;
        }
        if !needle.is_empty() {
            templates.retain(|t| t.name.to_lowercase().contains(&needle));
        }
        Ok(ListOutput {
            library: lib.map(|l| l.display().to_string()),
            folders: folder_list,
            templates,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewParams {
    /// Template reference from template.list, e.g. `starter:price-tag` or `my:Shop/Shelf`.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Name of the new document (default: the template's name).
    #[serde(default)]
    pub name: Option<String>,
}

/// `template.new`
#[derive(Debug)]
pub struct TemplateNew;

impl Command for TemplateNew {
    const ID: &'static str = "template.new";
    type Params = NewParams;
    type Output = OpenOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "New from template",
            "Create a new, unsaved document from a template (`ref` from template.list) and \
             make it active. Returns {doc_id, name}.",
        )
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: NewParams) -> Result<OpenOutput, CommandError> {
        let mut doc = load(ctx.session, &p.reference)?;
        if let TemplateRef::Library { name, .. } = TemplateRef::parse(&p.reference)? {
            doc.name = name;
        }
        if let Some(n) = p.name.filter(|n| !n.trim().is_empty()) {
            doc.name = n;
        }
        let name = doc.name.clone();
        let doc_id = ctx.session().state.add_doc(OpenDoc::new(doc, None));
        Ok(OpenOutput { doc_id, name })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RefOutput {
    /// Reference of the saved, moved or copied template.
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Write `doc` as library template `folder/name`.
fn write_template(
    lib: &Path,
    folder: &str,
    name: &str,
    doc: &Document,
    overwrite: bool,
) -> Result<RefOutput, CommandError> {
    let path = template_path(lib, folder, name);
    let r = TemplateRef::Library {
        folder: folder.to_owned(),
        name: name.to_owned(),
    };
    if path.exists() && !overwrite {
        return Err(CommandError::bad_params(format!(
            "template {} already exists",
            r.as_ref_string()
        ))
        .with_param("name")
        .with_hint("pick another name, or pass overwrite: true"));
    }
    let dir = folder_dir(lib, folder);
    std::fs::create_dir_all(&dir).map_err(|e| io_error("create", &dir, &e))?;
    let mut doc = doc.clone();
    doc.name = name.to_owned();
    file::save(&doc, &path).map_err(file_error)?;
    Ok(RefOutput {
        reference: r.as_ref_string(),
    })
}

/// First of `base`, `base 2`, `base 3`… not taken in `folder`.
fn free_name(lib: &Path, folder: &str, base: &str) -> String {
    (1..1000)
        .map(|i| {
            if i == 1 {
                base.to_owned()
            } else {
                format!("{base} {i}")
            }
        })
        .find(|n| !template_path(lib, folder, n).exists())
        .unwrap_or_else(|| base.to_owned())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Template name (default: the document's name).
    #[serde(default)]
    pub name: Option<String>,
    /// Library folder (default: the default folder); created when missing.
    #[serde(default)]
    pub folder: Option<String>,
    /// Replace a template of the same name (default false).
    #[serde(default)]
    pub overwrite: Option<bool>,
}

/// `template.save`
#[derive(Debug)]
pub struct TemplateSave;

impl Command for TemplateSave {
    const ID: &'static str = "template.save";
    type Params = SaveParams;
    type Output = RefOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Save as template",
            "Save a copy of the document (default: active) in the template library under \
             `name` (default: the document name) in `folder` (default folder when absent). \
             Fails if it exists unless overwrite is true. Returns {ref}.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: SaveParams) -> Result<RefOutput, CommandError> {
        let lib = library(ctx.session)?;
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let name = check_name("name", p.name.as_deref().unwrap_or(&doc.name))?;
        let folder = check_folder("folder", p.folder.as_deref().unwrap_or(""))?;
        write_template(&lib, &folder, &name, &doc, p.overwrite.unwrap_or(false))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefParams {
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteOutput {
    #[serde(rename = "ref")]
    pub reference: String,
}

fn library_ref(
    session: &Session,
    reference: &str,
    verb: &str,
) -> Result<(PathBuf, String, String), CommandError> {
    match TemplateRef::parse(reference)? {
        TemplateRef::Starter(_) => Err(CommandError::bad_params(format!(
            "starter templates are read-only and cannot be {verb}"
        ))
        .with_param("ref")
        .with_hint("copy it into the library with template.duplicate")),
        TemplateRef::Library { folder, name } => {
            let lib = library(session)?;
            if !template_path(&lib, &folder, &name).is_file() {
                return Err(CommandError::not_found(format!("no template {reference}"))
                    .with_param("ref")
                    .with_hint("run template.list for the references"));
            }
            Ok((lib, folder, name))
        }
    }
}

/// `template.delete`
#[derive(Debug)]
pub struct TemplateDelete;

impl Command for TemplateDelete {
    const ID: &'static str = "template.delete";
    type Params = RefParams;
    type Output = DeleteOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Delete template",
            "Delete a library template (`my:…`). Starters cannot be deleted. Cannot be undone.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: RefParams) -> Result<DeleteOutput, CommandError> {
        let (lib, folder, name) = library_ref(ctx.session, &p.reference, "deleted")?;
        let path = template_path(&lib, &folder, &name);
        std::fs::remove_file(&path).map_err(|e| io_error("delete", &path, &e))?;
        Ok(DeleteOutput {
            reference: p.reference,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveParams {
    #[serde(rename = "ref")]
    pub reference: String,
    /// New name (default: unchanged).
    #[serde(default)]
    pub name: Option<String>,
    /// Target folder, `""` for the default folder (default: unchanged); created when missing.
    #[serde(default)]
    pub folder: Option<String>,
}

/// `template.move`
#[derive(Debug)]
pub struct TemplateMove;

impl Command for TemplateMove {
    const ID: &'static str = "template.move";
    type Params = MoveParams;
    type Output = RefOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Rename or move template",
            "Rename a library template and/or move it to another folder. Fails if the target \
             exists. Returns the new {ref}.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: MoveParams) -> Result<RefOutput, CommandError> {
        let (lib, folder, name) = library_ref(ctx.session, &p.reference, "moved")?;
        let to_name = match &p.name {
            Some(n) => check_name("name", n)?,
            None => name.clone(),
        };
        let to_folder = match &p.folder {
            Some(f) => check_folder("folder", f)?,
            None => folder.clone(),
        };
        let from = template_path(&lib, &folder, &name);
        let to = template_path(&lib, &to_folder, &to_name);
        let r = TemplateRef::Library {
            folder: to_folder.clone(),
            name: to_name,
        };
        if from != to {
            // Case-only renames point at the same file on case-insensitive file systems.
            let same_file =
                from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
            if to.exists() && !same_file {
                return Err(CommandError::bad_params(format!(
                    "template {} already exists",
                    r.as_ref_string()
                ))
                .with_param("name"));
            }
            let dir = folder_dir(&lib, &to_folder);
            std::fs::create_dir_all(&dir).map_err(|e| io_error("create", &dir, &e))?;
            std::fs::rename(&from, &to).map_err(|e| io_error("move", &from, &e))?;
        }
        Ok(RefOutput {
            reference: r.as_ref_string(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DuplicateParams {
    /// Template to copy; starters may be copied into the library.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Name of the copy (default: the original's, numbered when taken).
    #[serde(default)]
    pub name: Option<String>,
    /// Folder of the copy (default: the original's; the default folder for starters).
    #[serde(default)]
    pub folder: Option<String>,
}

/// `template.duplicate`
#[derive(Debug)]
pub struct TemplateDuplicate;

impl Command for TemplateDuplicate {
    const ID: &'static str = "template.duplicate";
    type Params = DuplicateParams;
    type Output = RefOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Duplicate template",
            "Copy a template (a starter or a library one) into the library. Without `name` \
             the copy is numbered (`Name 2`). Returns the copy's {ref}.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: DuplicateParams) -> Result<RefOutput, CommandError> {
        let lib = library(ctx.session)?;
        let doc = load(ctx.session, &p.reference)?;
        let (src_folder, src_name) = match TemplateRef::parse(&p.reference)? {
            TemplateRef::Starter(_) => (String::new(), doc.name.clone()),
            TemplateRef::Library { folder, name } => (folder, name),
        };
        let folder = match &p.folder {
            Some(f) => check_folder("folder", f)?,
            None => src_folder,
        };
        let name = match &p.name {
            Some(n) => check_name("name", n)?,
            None => free_name(&lib, &folder, &check_name("name", &src_name)?),
        };
        write_template(&lib, &folder, &name, &doc, false)
    }
}

/// Folder operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FolderOp {
    Create,
    Rename,
    Delete,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderParams {
    pub op: FolderOp,
    /// Folder to create, rename or delete.
    pub folder: String,
    /// New name, for `rename`.
    #[serde(default)]
    pub to: Option<String>,
    /// `delete`: also delete the templates in it (default false: it must be empty).
    #[serde(default)]
    pub with_templates: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FolderOutput {
    /// Library folders after the change.
    pub folders: Vec<String>,
}

/// `template.folder`
#[derive(Debug)]
pub struct TemplateFolder;

impl Command for TemplateFolder {
    const ID: &'static str = "template.folder";
    type Params = FolderParams;
    type Output = FolderOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Template folder",
            "Create, rename or delete a template library folder (`op`). Delete needs an empty \
             folder unless with_templates is true. Returns the folders.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: FolderParams) -> Result<FolderOutput, CommandError> {
        let lib = library(ctx.session)?;
        let name = check_name("folder", &p.folder)?;
        let dir = lib.join(&name);
        let missing = || {
            CommandError::not_found(format!("no template folder `{name}`"))
                .with_param("folder")
                .with_accepted(folders(&lib))
        };
        match p.op {
            FolderOp::Create => {
                if dir.exists() {
                    return Err(CommandError::bad_params(format!(
                        "folder `{name}` already exists"
                    ))
                    .with_param("folder"));
                }
                std::fs::create_dir_all(&dir).map_err(|e| io_error("create", &dir, &e))?;
            }
            FolderOp::Rename => {
                let to = check_name(
                    "to",
                    p.to.as_deref().ok_or_else(|| {
                        CommandError::bad_params("`rename` needs `to`").with_param("to")
                    })?,
                )?;
                if !dir.is_dir() {
                    return Err(missing());
                }
                let target = lib.join(&to);
                let same = to.to_lowercase() == name.to_lowercase();
                if target.exists() && !same {
                    return Err(
                        CommandError::bad_params(format!("folder `{to}` already exists"))
                            .with_param("to"),
                    );
                }
                std::fs::rename(&dir, &target).map_err(|e| io_error("rename", &dir, &e))?;
            }
            FolderOp::Delete => {
                if !dir.is_dir() {
                    return Err(missing());
                }
                let count = names_in(&dir).len();
                if count > 0 && !p.with_templates.unwrap_or(false) {
                    return Err(CommandError::bad_params(format!(
                        "folder `{name}` holds {count} template(s)"
                    ))
                    .with_param("folder")
                    .with_hint("move them out first, or pass with_templates: true"));
                }
                std::fs::remove_dir_all(&dir).map_err(|e| io_error("delete", &dir, &e))?;
            }
        }
        Ok(FolderOutput {
            folders: folders(&lib),
        })
    }
}
