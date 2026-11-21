//! `paper.*`: the label paper library and RFID paper detection (ADR-0020).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::check_mm;
use super::device::pick_address;
use super::doc::validate_label;
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{LabelSpec, MediaType};
use crate::paper::{CUSTOM_PREFIX, Paper, UserPapers};
use crate::session::Session;

/// Longest paper name, in characters.
const MAX_NAME: usize = 80;
/// Longest RFID barcode, in characters.
const MAX_BARCODE: usize = 64;

/// A paper as listed: the paper plus where it comes from and its linked barcodes.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PaperEntry {
    #[serde(flatten)]
    pub paper: Paper,
    /// True for the user's own papers (`my:`), false for bundled ones (`std:`).
    pub custom: bool,
    /// RFID barcodes linked to this paper.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub barcodes: Vec<String>,
}

fn entry(lib: &UserPapers, p: &Paper) -> PaperEntry {
    PaperEntry {
        paper: p.clone(),
        custom: p.is_custom(),
        barcodes: lib.barcodes_of(&p.id),
    }
}

fn paper_file(session: &Session) -> Result<std::path::PathBuf, CommandError> {
    session
        .paper_file()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| {
            CommandError::disabled("no paper library file is configured in this session")
                .with_hint("bundled papers still work; the studio and CLI set a library")
        })
}

fn find<'a>(lib: &'a UserPapers, id: &str) -> Result<&'a Paper, CommandError> {
    lib.get(id).ok_or_else(|| {
        CommandError::not_found(format!("no paper `{id}`"))
            .with_param("id")
            .with_hint("list papers with paper.list")
    })
}

fn check_barcode(barcode: &str) -> Result<String, CommandError> {
    let b = barcode.trim();
    if b.is_empty() || b.chars().count() > MAX_BARCODE || b.chars().any(char::is_control) {
        return Err(CommandError::bad_params(format!(
            "barcode must be 1–{MAX_BARCODE} printable characters"
        ))
        .with_param("barcode"));
    }
    Ok(b.to_owned())
}

fn matches_query(lib: &UserPapers, p: &Paper, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let size = format!("{}x{}", p.width_mm, p.height_mm);
    p.id.to_lowercase().contains(&q)
        || p.name.to_lowercase().contains(&q)
        || size.contains(&q.replace(['×', ' '], "").replace('*', "x"))
        || lib
            .barcodes_of(&p.id)
            .iter()
            .any(|b| b.contains(q.as_str()))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// Search in id, name, size (`50x30`) and linked RFID barcodes.
    #[serde(default)]
    pub query: Option<String>,
    /// Only papers that fit this printer model (width ≤ its print width), e.g.
    /// `niimbot.b21_pro`.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub media: Option<MediaType>,
    /// Only the user's papers (true) or only bundled ones (false).
    #[serde(default)]
    pub custom: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListOutput {
    pub papers: Vec<PaperEntry>,
}

/// `paper.list`
#[derive(Debug)]
pub struct PaperList;

impl Command for PaperList {
    const ID: &'static str = "paper.list";
    type Params = ListParams;
    type Output = ListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Label papers",
            "List label papers: Labely's bundled sizes (`std:…`) and the user's own (`my:…`), \
             with any RFID barcodes linked to them. Filter by query (id, name, `50x30`, \
             barcode), model (fits the printer's width), media and custom. Sizes in mm.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ListParams) -> Result<ListOutput, CommandError> {
        let lib = ctx.session().user_papers()?;
        let max_width = match &p.model {
            None => None,
            Some(id) => {
                let models = ctx.services().devices.models();
                let Some(m) = models.iter().find(|m| &m.id == id) else {
                    return Err(CommandError::not_found(format!("no printer model `{id}`"))
                        .with_param("model")
                        .with_accepted(models.iter().map(|m| m.id.clone())));
                };
                Some(m.max_width_mm)
            }
        };
        let papers = lib
            .all()
            .filter(|x| max_width.is_none_or(|w| x.width_mm <= w + 0.01))
            .filter(|x| p.media.is_none_or(|m| x.media == m))
            .filter(|x| p.custom.is_none_or(|c| x.is_custom() == c))
            .filter(|x| matches_query(&lib, x, p.query.as_deref().unwrap_or("")))
            .map(|x| entry(&lib, x))
            .collect();
        Ok(ListOutput { papers })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddParams {
    pub name: String,
    /// Width across the printhead, mm.
    pub width_mm: f64,
    /// Length along the feed, mm.
    pub height_mm: f64,
    #[serde(default)]
    pub corner_radius_mm: Option<f64>,
    /// Default `gap`.
    #[serde(default)]
    pub media: Option<MediaType>,
    /// Also link this RFID barcode to the new paper.
    #[serde(default)]
    pub barcode: Option<String>,
}

/// `paper.add`
#[derive(Debug)]
pub struct PaperAdd;

impl Command for PaperAdd {
    const ID: &'static str = "paper.add";
    type Params = AddParams;
    type Output = PaperEntry;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add label paper",
            "Add a paper to the user's library. Sizes in mm; media defaults to gap. Returns \
             the paper with its new `my:` id. With `barcode`, also links that RFID barcode to \
             it. Not undoable; remove it with paper.delete.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: AddParams) -> Result<PaperEntry, CommandError> {
        let file = paper_file(ctx.session())?;
        let name = p.name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(
                CommandError::bad_params(format!("name must be 1–{MAX_NAME} characters"))
                    .with_param("name"),
            );
        }
        let label = LabelSpec {
            width_mm: check_mm("width_mm", p.width_mm, true)?,
            height_mm: check_mm("height_mm", p.height_mm, true)?,
            corner_radius_mm: p.corner_radius_mm.unwrap_or(0.0),
            ..LabelSpec::default()
        };
        validate_label(&label)?;
        let barcode = p.barcode.as_deref().map(check_barcode).transpose()?;
        let mut lib = UserPapers::load(&file)?;
        let paper = Paper {
            id: lib.new_id(name),
            name: name.to_owned(),
            width_mm: label.width_mm,
            height_mm: label.height_mm,
            corner_radius_mm: label.corner_radius_mm,
            media: p.media.unwrap_or_default(),
        };
        lib.papers.push(paper.clone());
        if let Some(b) = barcode {
            lib.links.insert(b, paper.id.clone());
        }
        lib.save(&file)?;
        Ok(entry(&lib, &paper))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdParams {
    /// Paper id from paper.list.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteOutput {
    pub id: String,
    /// Barcode links removed with it.
    pub unlinked: Vec<String>,
}

/// `paper.delete`
#[derive(Debug)]
pub struct PaperDelete;

impl Command for PaperDelete {
    const ID: &'static str = "paper.delete";
    type Params = IdParams;
    type Output = DeleteOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Delete label paper",
            "Delete one of the user's papers (`my:…`) and its barcode links. Bundled papers \
             cannot be deleted. Cannot be undone.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: IdParams) -> Result<DeleteOutput, CommandError> {
        if !p.id.starts_with(CUSTOM_PREFIX) {
            return Err(CommandError::bad_params(format!(
                "`{}` is a bundled paper; only `my:` papers can be deleted",
                p.id
            ))
            .with_param("id"));
        }
        let file = paper_file(ctx.session())?;
        let mut lib = UserPapers::load(&file)?;
        find(&lib, &p.id)?;
        lib.papers.retain(|x| x.id != p.id);
        let unlinked = lib.barcodes_of(&p.id);
        lib.links.retain(|_, id| *id != p.id);
        lib.save(&file)?;
        Ok(DeleteOutput { id: p.id, unlinked })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkParams {
    /// RFID barcode, as reported by device.status or paper.detect.
    pub barcode: String,
    /// Paper id to link it to (bundled or user).
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LinkOutput {
    pub barcode: String,
    /// Linked paper id; absent after paper.unlink.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The paper it was linked to before, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

/// `paper.link`
#[derive(Debug)]
pub struct PaperLink;

impl Command for PaperLink {
    const ID: &'static str = "paper.link";
    type Params = LinkParams;
    type Output = LinkOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Link RFID paper",
            "Remember that rolls with this RFID barcode are paper `id`, so paper.detect and \
             doc.check recognise them. Replaces an existing link. Not undoable.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: LinkParams) -> Result<LinkOutput, CommandError> {
        let barcode = check_barcode(&p.barcode)?;
        let file = paper_file(ctx.session())?;
        let mut lib = UserPapers::load(&file)?;
        find(&lib, &p.id)?;
        let previous = lib.links.insert(barcode.clone(), p.id.clone());
        lib.save(&file)?;
        Ok(LinkOutput {
            barcode,
            id: Some(p.id),
            previous,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnlinkParams {
    pub barcode: String,
}

/// `paper.unlink`
#[derive(Debug)]
pub struct PaperUnlink;

impl Command for PaperUnlink {
    const ID: &'static str = "paper.unlink";
    type Params = UnlinkParams;
    type Output = LinkOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Unlink RFID paper",
            "Forget an RFID barcode link. Not undoable.",
        )
        .undoable(false)
        .journal(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: UnlinkParams) -> Result<LinkOutput, CommandError> {
        let barcode = p.barcode.trim().to_owned();
        let file = paper_file(ctx.session())?;
        let mut lib = UserPapers::load(&file)?;
        let Some(previous) = lib.links.remove(&barcode) else {
            let e = CommandError::not_found(format!("barcode `{barcode}` is not linked"))
                .with_param("barcode")
                .with_accepted(lib.links.keys().cloned());
            // A bundled link (ADR-0041) cannot be removed, only overridden.
            return Err(if crate::paper::builtin_links().contains_key(&barcode) {
                e.with_hint("this barcode is in the bundled table; use paper.link to point it at another paper")
            } else {
                e
            });
        };
        lib.save(&file)?;
        Ok(LinkOutput {
            barcode,
            id: None,
            previous: Some(previous),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Paper id from paper.list or paper.detect.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApplyOutput {
    pub label: LabelSpec,
}

/// `paper.apply`
#[derive(Debug)]
pub struct PaperApply;

impl Command for PaperApply {
    const ID: &'static str = "paper.apply";
    type Params = ApplyParams;
    type Output = ApplyOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Use label paper",
            "Set the label's width, height, corner radius and media from a paper. Elements \
             keep their positions. Undoable.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ApplyParams) -> Result<ApplyOutput, CommandError> {
        let lib = ctx.session().user_papers()?;
        let paper = find(&lib, &p.id)?.clone();
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let mut label = doc.label.clone();
        label.width_mm = paper.width_mm;
        label.height_mm = paper.height_mm;
        label.corner_radius_mm = paper.corner_radius_mm;
        label.media = paper.media;
        validate_label(&label)?;
        doc.label = label.clone();
        Ok(ApplyOutput { label })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DetectParams {
    /// Printer address; defaults to the single connected printer.
    #[serde(default)]
    pub address: Option<String>,
    /// Document to compare with (default: the active one, if any).
    #[serde(default)]
    pub doc_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DetectOutput {
    pub address: String,
    /// False when the printer reported no RFID tag (no paper, or paper without a tag).
    pub rfid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Media type the tag reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaType>,
    /// Labels left on the roll (total − used), as the tag reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<u32>,
    /// The linked paper; absent when the barcode is not linked yet (use paper.link).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper: Option<PaperEntry>,
    /// Whether the document's label matches the paper (size and media), or only the media
    /// when the paper is unknown. Absent without a document or tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_label: Option<bool>,
}

/// `paper.detect`
#[derive(Debug)]
pub struct PaperDetect;

impl Command for PaperDetect {
    const ID: &'static str = "paper.detect";
    type Params = DetectParams;
    type Output = DetectOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Detect label paper",
            "Read the loaded roll's RFID tag and look its barcode up in the paper library. \
             Returns {rfid, barcode, media, remaining, paper?, matches_label?}. When paper is \
             found and matches_label is false, paper.apply {id} sets the label to it. An \
             unknown barcode can be linked with paper.link.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DetectParams) -> Result<DetectOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        let status = ctx.services().devices.status(&address)?;
        let label = match &p.doc_id {
            Some(id) => Some(ctx.doc(Some(id))?.label.clone()),
            None => ctx.doc(None).ok().map(|d| d.label.clone()),
        };
        let Some(info) = status.paper else {
            return Ok(DetectOutput {
                address,
                rfid: false,
                barcode: None,
                media: None,
                remaining: None,
                paper: None,
                matches_label: None,
            });
        };
        let lib = ctx.session().user_papers()?;
        let paper = lib.by_barcode(&info.barcode).map(|x| entry(&lib, x));
        let matches_label = label.as_ref().map(|l| match &paper {
            Some(e) => e.paper.matches(l),
            None => info.media.is_none_or(|m| m == l.media),
        });
        Ok(DetectOutput {
            address,
            rfid: true,
            remaining: Some(info.total.saturating_sub(info.used)),
            barcode: Some(info.barcode),
            media: info.media,
            paper,
            matches_label,
        })
    }
}

/// The paper linked to `barcode` in the session's library (errors read as unknown).
pub(crate) fn linked_paper(session: &Session, barcode: &str) -> Option<Paper> {
    session
        .user_papers()
        .ok()
        .and_then(|lib| lib.by_barcode(barcode).cloned())
}
