//! Arranging: `element.align`, `element.distribute`, `element.group`, `element.ungroup`,
//! `element.lock`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandError, CommandMeta, Ctx, element_not_found};
use crate::document::{Document, ElementId};

/// Edge or centre line to align on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlignEdge {
    Left,
    /// Horizontal centres.
    Center,
    Right,
    Top,
    /// Vertical centres.
    Middle,
    Bottom,
}

/// What `element.align` aligns to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlignTo {
    /// The bounding box of the given elements.
    Selection,
    /// The label.
    Label,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlignParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub ids: Vec<ElementId>,
    pub edge: AlignEdge,
    /// Default: `selection` for two or more elements, `label` for one.
    #[serde(default)]
    pub to: Option<AlignTo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MovedOutput {
    /// Elements whose position changed.
    pub moved: Vec<ElementId>,
}

/// Ids that exist, de-duplicated, in order; locked ones are dropped with a warning.
fn movable(
    ctx: &mut Ctx<'_>,
    doc: &Document,
    ids: &[ElementId],
) -> Result<Vec<ElementId>, CommandError> {
    let mut out = Vec::new();
    let mut locked = Vec::new();
    for id in ids {
        let e = doc
            .element(*id)
            .ok_or_else(|| element_not_found(doc, *id))?;
        if out.contains(id) || locked.contains(id) {
            continue;
        }
        if e.locked {
            locked.push(*id);
        } else {
            out.push(*id);
        }
    }
    if !locked.is_empty() {
        ctx.warn(format!(
            "locked element(s) {} were not moved",
            locked
                .iter()
                .map(|i| i.0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(out)
}

fn union(doc: &Document, ids: &[ElementId]) -> (f64, f64, f64, f64) {
    ids.iter()
        .filter_map(|id| doc.element(*id))
        .map(|e| e.bounds())
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, b| {
            (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
        })
}

/// Move element `id` by (dx, dy); returns whether it moved.
fn shift(doc: &mut Document, id: ElementId, dx: f64, dy: f64) -> bool {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    match doc.element_mut(id) {
        Some(e) if dx.abs() > 1e-9 || dy.abs() > 1e-9 => {
            e.x_mm = round(e.x_mm + dx);
            e.y_mm = round(e.y_mm + dy);
            true
        }
        _ => false,
    }
}

/// `element.align`
#[derive(Debug)]
pub struct Align;

impl Command for Align {
    const ID: &'static str = "element.align";
    type Params = AlignParams;
    type Output = MovedOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Align",
            "Align elements' rotated bounding boxes on an edge or centre line (left, center, \
             right, top, middle, bottom) of their common box (`to: selection`, the default for \
             2+ ids) or of the label. Locked elements stay put. Returns {moved}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AlignParams) -> Result<MovedOutput, CommandError> {
        let doc_arc = ctx.doc_arc(p.doc_id.as_deref())?;
        if p.ids.is_empty() {
            return Err(CommandError::bad_params("pass at least one id").with_param("ids"));
        }
        let ids = movable(ctx, &doc_arc, &p.ids)?;
        let to = p.to.unwrap_or(if p.ids.len() > 1 {
            AlignTo::Selection
        } else {
            AlignTo::Label
        });
        let target = match to {
            AlignTo::Label => (0.0, 0.0, doc_arc.label.width_mm, doc_arc.label.height_mm),
            AlignTo::Selection => union(&doc_arc, &p.ids),
        };
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let mut moved = Vec::new();
        for id in ids {
            let Some(b) = doc.element(id).map(|e| e.bounds()) else {
                continue;
            };
            let (dx, dy) = match p.edge {
                AlignEdge::Left => (target.0 - b.0, 0.0),
                AlignEdge::Right => (target.2 - b.2, 0.0),
                AlignEdge::Center => ((target.0 + target.2 - b.0 - b.2) / 2.0, 0.0),
                AlignEdge::Top => (0.0, target.1 - b.1),
                AlignEdge::Bottom => (0.0, target.3 - b.3),
                AlignEdge::Middle => (0.0, (target.1 + target.3 - b.1 - b.3) / 2.0),
            };
            if shift(doc, id, dx, dy) {
                moved.push(id);
            }
        }
        Ok(MovedOutput { moved })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DistributeParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Three or more elements.
    pub ids: Vec<ElementId>,
    pub axis: Axis,
}

/// `element.distribute`
#[derive(Debug)]
pub struct Distribute;

impl Command for Distribute {
    const ID: &'static str = "element.distribute";
    type Params = DistributeParams;
    type Output = MovedOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Distribute",
            "Space 3+ elements evenly along an axis: the outermost stay, the others move so the \
             gaps between their bounding boxes are equal. Returns {moved}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DistributeParams) -> Result<MovedOutput, CommandError> {
        let doc_arc = ctx.doc_arc(p.doc_id.as_deref())?;
        let ids = movable(ctx, &doc_arc, &p.ids)?;
        if ids.len() < 3 {
            return Err(CommandError::bad_params(
                "distribute needs at least three unlocked elements",
            )
            .with_param("ids"));
        }
        let h = p.axis == Axis::Horizontal;
        let mut items: Vec<(ElementId, f64, f64)> = ids
            .iter()
            .filter_map(|id| doc_arc.element(*id))
            .map(|e| {
                let b = e.bounds();
                if h {
                    (e.id, b.0, b.2)
                } else {
                    (e.id, b.1, b.3)
                }
            })
            .collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        let start = items[0].1;
        let end = items.iter().map(|i| i.2).fold(f64::MIN, f64::max);
        let total: f64 = items.iter().map(|i| i.2 - i.1).sum();
        let gap = (end - start - total) / (items.len() - 1) as f64;
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let mut pos = start;
        let mut moved = Vec::new();
        let last = items.len() - 1;
        for (i, (id, lo, hi)) in items.into_iter().enumerate() {
            let d = pos - lo;
            if i != 0 && i != last {
                let (dx, dy) = if h { (d, 0.0) } else { (0.0, d) };
                if shift(doc, id, dx, dy) {
                    moved.push(id);
                }
            }
            pos += hi - lo + gap;
        }
        Ok(MovedOutput { moved })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdsParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub ids: Vec<ElementId>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GroupOutput {
    pub group: u64,
    /// Every member of the new group.
    pub members: Vec<ElementId>,
}

/// Every element of the groups `ids` belong to, plus the ungrouped ids themselves.
pub fn expand_groups(doc: &Document, ids: &[ElementId]) -> Vec<ElementId> {
    let groups: Vec<u64> = ids
        .iter()
        .filter_map(|id| doc.element(*id).and_then(|e| e.group))
        .collect();
    let mut out: Vec<ElementId> = Vec::new();
    for e in doc.elements() {
        if ids.contains(&e.id) || e.group.is_some_and(|g| groups.contains(&g)) {
            out.push(e.id);
        }
    }
    out
}

/// `element.group`
#[derive(Debug)]
pub struct Group;

impl Command for Group {
    const ID: &'static str = "element.group";
    type Params = IdsParams;
    type Output = GroupOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Group",
            "Group two or more elements (members of existing groups bring their whole group). \
             Groups are flat; the studio selects and moves a group as one. Returns {group, \
             members}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: IdsParams) -> Result<GroupOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        for id in &p.ids {
            if doc.element(*id).is_none() {
                return Err(element_not_found(doc, *id));
            }
        }
        let members = expand_groups(doc, &p.ids);
        if members.len() < 2 {
            return Err(
                CommandError::bad_params("a group needs at least two elements").with_param("ids"),
            );
        }
        let group = doc.next_id;
        doc.next_id += 1;
        for id in &members {
            if let Some(e) = doc.element_mut(*id) {
                e.group = Some(group);
            }
        }
        Ok(GroupOutput { group, members })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UngroupOutput {
    /// Elements that left a group.
    pub ungrouped: Vec<ElementId>,
}

/// `element.ungroup`
#[derive(Debug)]
pub struct Ungroup;

impl Command for Ungroup {
    const ID: &'static str = "element.ungroup";
    type Params = IdsParams;
    type Output = UngroupOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Ungroup",
            "Dissolve the groups of the given elements. Returns {ungrouped}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: IdsParams) -> Result<UngroupOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        for id in &p.ids {
            if doc.element(*id).is_none() {
                return Err(element_not_found(doc, *id));
            }
        }
        let members: Vec<ElementId> = expand_groups(doc, &p.ids)
            .into_iter()
            .filter(|id| doc.element(*id).is_some_and(|e| e.group.is_some()))
            .collect();
        for id in &members {
            if let Some(e) = doc.element_mut(*id) {
                e.group = None;
            }
        }
        if members.is_empty() {
            ctx.warn("none of the elements is grouped");
        }
        Ok(UngroupOutput { ungrouped: members })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LockParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub ids: Vec<ElementId>,
    /// Lock (default) or unlock.
    #[serde(default)]
    pub locked: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LockOutput {
    pub ids: Vec<ElementId>,
    pub locked: bool,
}

/// `element.lock`
#[derive(Debug)]
pub struct Lock;

impl Command for Lock {
    const ID: &'static str = "element.lock";
    type Params = LockParams;
    type Output = LockOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Lock",
            "Lock (default) or unlock elements. Locked elements cannot be moved, resized or \
             edited until unlocked.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: LockParams) -> Result<LockOutput, CommandError> {
        let locked = p.locked.unwrap_or(true);
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        for id in &p.ids {
            let e = match doc.element_mut(*id) {
                Some(e) => e,
                None => return Err(element_not_found(doc, *id)),
            };
            e.locked = locked;
        }
        Ok(LockOutput { ids: p.ids, locked })
    }
}
