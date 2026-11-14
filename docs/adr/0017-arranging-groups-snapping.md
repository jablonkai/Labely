# ADR-0017: Arranging, groups, locking and snapping

- Status: proposed
- Date: 2026-11-14

## Context
Editor slice 3.3 of the feature matrix: align, distribute, group, lock, snapping, a grid,
shortcuts and a canvas background. Agents need the arranging operations as commands. Snapping
and the grid are pointer aids, so they only matter in the studio.

## Decision
- **Bounds.** Arranging uses the axis-aligned box of the rotated frame (`Element::bounds`).
  `doc.check` overlap and bounds rules use the same box.
- **Commands.** All are `edit` commands, and each is one undo step:
  - `element.align {ids, edge, to}`. `to` defaults to the selection's box for 2+ ids and to
    the label for one id.
  - `element.distribute {ids, axis}`: the outermost elements stay put, and the gaps between
    the others become equal.
  - `element.group` / `element.ungroup`.
  - `element.lock {ids, locked}`.
  - Locked elements are skipped with a warning. They do not cause an error.
- **Groups.** Groups are flat: an element has `group: Option<u64>`, and ids come from the
  element id counter. Grouping a member of an existing group pulls in its whole group, so
  groups never nest. A duplicate leaves the group. The studio widens every selection to whole
  groups, so a click or drag works on the whole group.
- **Snapping (studio).** While dragging, the edges and centre of the moving box snap to the
  label's edges and centre and to other elements' edges and centres, within 6 screen px.
  Pink guides show the line it snapped to. If no line is close and the grid is shown, the
  left and top edges snap to the grid instead. Alt drags freely, and Arrange → Snap turns
  snapping off.
- **View aids (studio, not saved).** View → Show grid with a spacing in mm, and View → Label
  colour, which tints the canvas to match coloured label stock. Neither changes the document
  or what is printed.
- **Shortcuts.** Ctrl+G groups, Ctrl+Shift+G ungroups, Ctrl+L locks or unlocks (it unlocks
  when every selected element is locked). Arrow keys still nudge by 0.5 mm, and Shift+arrow
  by 5 mm.
- **Format.** `group` is a new optional field in the unreleased v2, so the version stays.

## Consequences
- Nested groups and group-level transforms (rotate or resize a group as one) are not supported.
  Resize and rotate handles still need a single selected element.
- A background image (tracing a scanned label) is left for later, together with image import
  (3.4).
