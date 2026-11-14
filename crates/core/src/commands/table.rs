//! Tables: `element.add_table`, `element.table_edit`.

use schemars::JsonSchema;
use serde::Deserialize;

use super::element::{AddOutput, ElementOutput, Frame, add, add_params, validate_element};
use crate::command::{Command, CommandError, CommandMeta, Ctx, element_mut};
use crate::document::{
    CellRange, ElementId, ElementKind, HAlign, MAX_TABLE_DIM, TableCell, TableProps,
};

add_params!(TableParams {
    /// Number of rows, 1–64 (default 3; ignored when `cells` is given).
    #[serde(default)]
    row_count: Option<usize>,
    /// Number of columns, 1–64 (default 3; ignored when `cells` is given).
    #[serde(default)]
    column_count: Option<usize>,
    /// Initial cell texts, row-major; short rows are padded with empty cells.
    #[serde(default)]
    cells: Option<Vec<Vec<String>>>,
    /// Grid line width in mm (default 0.3; 0 hides the lines).
    #[serde(default)]
    stroke_mm: Option<f64>,
    /// Default cell font size in points (default 8); text shrinks to fit its cell.
    #[serde(default)]
    font_size_pt: Option<f64>,
    #[serde(default)]
    font_family: Option<String>,
    /// Default cell alignment (default `center`).
    #[serde(default)]
    align: Option<HAlign>,
});

/// `element.add_table`
#[derive(Debug)]
pub struct AddTable;

impl Command for AddTable {
    const ID: &'static str = "element.add_table";
    type Params = TableParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Table",
            "Add a table that fills its frame: equal rows and columns (row_count × \
             column_count, default 3×3), or sized from `cells` (row-major texts, `{{name}}` \
             bindings allowed). Edit it with element.table_edit. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: TableParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let mut t = match &p.cells {
            Some(rows) => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
                let mut t = TableProps::new(rows.len().max(1), cols);
                for (r, row) in rows.iter().enumerate() {
                    for (c, text) in row.iter().enumerate() {
                        t.cells[r][c].text.clone_from(text);
                    }
                }
                t
            }
            None => TableProps::new(p.row_count.unwrap_or(3), p.column_count.unwrap_or(3)),
        };
        if let Some(v) = p.stroke_mm {
            t.stroke_mm = v;
        }
        if let Some(v) = p.font_size_pt {
            t.font_size_pt = v;
        }
        if let Some(v) = p.font_family {
            t.font_family = v;
        }
        if let Some(v) = p.align {
            t.align = v;
        }
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Table(t))
    }
}

/// One edit of a table. Indices are 0-based.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum TableOp {
    /// Insert an empty row before `at` (`at` = row count appends).
    InsertRow {
        at: usize,
        /// Relative height (default: the neighbour's).
        #[serde(default)]
        height: Option<f64>,
    },
    DeleteRow {
        at: usize,
    },
    /// Insert an empty column before `at` (`at` = column count appends).
    InsertColumn {
        at: usize,
        /// Relative width (default: the neighbour's).
        #[serde(default)]
        width: Option<f64>,
    },
    DeleteColumn {
        at: usize,
    },
    /// Change a cell's text or style; absent fields stay as they are.
    SetCell {
        row: usize,
        col: usize,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        bold: Option<bool>,
        #[serde(default)]
        align: Option<HAlign>,
        #[serde(default)]
        font_size_pt: Option<f64>,
        #[serde(default)]
        invert: Option<bool>,
    },
    /// Merge `rows × cols` cells from (row, col); merges inside the block are replaced.
    Merge {
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
    },
    /// Undo the merge covering (row, col).
    Split {
        row: usize,
        col: usize,
    },
    /// Set the relative height of a row.
    ResizeRow {
        at: usize,
        height: f64,
    },
    /// Set the relative width of a column.
    ResizeColumn {
        at: usize,
        width: f64,
    },
    /// Empty every cell's text (styles stay).
    Clear,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TableEditParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Table element id.
    pub id: ElementId,
    /// The edit, tagged by `op`.
    pub edit: TableOp,
}

/// `element.table_edit`
#[derive(Debug)]
pub struct TableEdit;

fn out_of_range(what: &str, i: usize, n: usize) -> CommandError {
    CommandError::bad_params(format!(
        "{what} {i} is out of range 0–{}",
        n.saturating_sub(1)
    ))
    .with_param("edit")
    .with_hint("table indices are 0-based")
}

/// Shift or shrink merges for a row/column inserted (`delta` = 1) or deleted (-1) at `at`.
fn adjust_merges(merges: &mut Vec<CellRange>, rows_axis: bool, at: usize, insert: bool) {
    for m in merges.iter_mut() {
        let (start, len) = if rows_axis {
            (&mut m.row, &mut m.rows)
        } else {
            (&mut m.col, &mut m.cols)
        };
        if insert {
            if at <= *start {
                *start += 1;
            } else if at < *start + *len {
                *len += 1;
            }
        } else if at < *start {
            *start -= 1;
        } else if at < *start + *len {
            *len -= 1;
        }
    }
    merges.retain(|m| m.rows > 0 && m.cols > 0 && m.rows * m.cols >= 2);
}

fn apply(t: &mut TableProps, op: TableOp) -> Result<(), CommandError> {
    let (rows, cols) = (t.row_heights.len(), t.column_widths.len());
    match op {
        TableOp::InsertRow { at, height } => {
            if at > rows {
                return Err(out_of_range("row", at, rows + 1));
            }
            if rows >= MAX_TABLE_DIM {
                return Err(CommandError::bad_params(format!(
                    "a table has at most {MAX_TABLE_DIM} rows"
                )));
            }
            let h = height.unwrap_or(t.row_heights[at.min(rows - 1)]);
            t.row_heights.insert(at, h);
            t.cells.insert(at, vec![TableCell::default(); cols]);
            adjust_merges(&mut t.merges, true, at, true);
        }
        TableOp::DeleteRow { at } => {
            if at >= rows {
                return Err(out_of_range("row", at, rows));
            }
            if rows == 1 {
                return Err(CommandError::bad_params(
                    "cannot delete the last row; delete the element instead",
                ));
            }
            t.row_heights.remove(at);
            t.cells.remove(at);
            adjust_merges(&mut t.merges, true, at, false);
        }
        TableOp::InsertColumn { at, width } => {
            if at > cols {
                return Err(out_of_range("column", at, cols + 1));
            }
            if cols >= MAX_TABLE_DIM {
                return Err(CommandError::bad_params(format!(
                    "a table has at most {MAX_TABLE_DIM} columns"
                )));
            }
            let w = width.unwrap_or(t.column_widths[at.min(cols - 1)]);
            t.column_widths.insert(at, w);
            for r in &mut t.cells {
                r.insert(at, TableCell::default());
            }
            adjust_merges(&mut t.merges, false, at, true);
        }
        TableOp::DeleteColumn { at } => {
            if at >= cols {
                return Err(out_of_range("column", at, cols));
            }
            if cols == 1 {
                return Err(CommandError::bad_params(
                    "cannot delete the last column; delete the element instead",
                ));
            }
            t.column_widths.remove(at);
            for r in &mut t.cells {
                r.remove(at);
            }
            adjust_merges(&mut t.merges, false, at, false);
        }
        TableOp::SetCell {
            row,
            col,
            text,
            bold,
            align,
            font_size_pt,
            invert,
        } => {
            if row >= rows {
                return Err(out_of_range("row", row, rows));
            }
            if col >= cols {
                return Err(out_of_range("column", col, cols));
            }
            let c = &mut t.cells[row][col];
            if let Some(v) = text {
                c.text = v;
            }
            if let Some(v) = bold {
                c.bold = v;
            }
            if let Some(v) = align {
                c.align = Some(v);
            }
            if let Some(v) = font_size_pt {
                c.font_size_pt = Some(v);
            }
            if let Some(v) = invert {
                c.invert = v;
            }
        }
        TableOp::Merge {
            row,
            col,
            rows: rs,
            cols: cs,
        } => {
            let m = CellRange {
                row,
                col,
                rows: rs,
                cols: cs,
            };
            if rs == 0 || cs == 0 || rs * cs < 2 {
                return Err(
                    CommandError::bad_params("a merge spans at least two cells").with_param("edit")
                );
            }
            if row + rs > rows || col + cs > cols {
                return Err(CommandError::bad_params(format!(
                    "the merge extends outside the {rows}×{cols} table"
                ))
                .with_param("edit"));
            }
            let inside = |o: &CellRange| {
                o.row >= row
                    && o.col >= col
                    && o.row + o.rows <= row + rs
                    && o.col + o.cols <= col + cs
            };
            if t.merges.iter().any(|o| o.overlaps(&m) && !inside(o)) {
                return Err(CommandError::bad_params(
                    "the merge partly overlaps another merge; split that one first",
                )
                .with_param("edit"));
            }
            t.merges.retain(|o| !inside(o));
            t.merges.push(m);
        }
        TableOp::Split { row, col } => {
            let before = t.merges.len();
            t.merges.retain(|m| !m.contains(row, col));
            if t.merges.len() == before {
                return Err(
                    CommandError::not_found(format!("cell ({row}, {col}) is not merged"))
                        .with_param("edit"),
                );
            }
        }
        TableOp::ResizeRow { at, height } => {
            let h = t
                .row_heights
                .get_mut(at)
                .ok_or_else(|| out_of_range("row", at, rows))?;
            *h = height;
        }
        TableOp::ResizeColumn { at, width } => {
            let w = t
                .column_widths
                .get_mut(at)
                .ok_or_else(|| out_of_range("column", at, cols))?;
            *w = width;
        }
        TableOp::Clear => {
            for c in t.cells.iter_mut().flatten() {
                c.text.clear();
            }
        }
    }
    Ok(())
}

impl Command for TableEdit {
    const ID: &'static str = "element.table_edit";
    type Params = TableEditParams;
    type Output = ElementOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Edit table",
            "One table edit, tagged by `op`: insert_row/delete_row {at}, insert_column/\
             delete_column {at}, set_cell {row, col, text, bold, align, font_size_pt, invert}, \
             merge {row, col, rows, cols}, split {row, col}, resize_row {at, height}, \
             resize_column {at, width} (relative weights), clear. Indices are 0-based. \
             Returns {element}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: TableEditParams) -> Result<ElementOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let e = element_mut(doc, p.id)?;
        if e.locked {
            return Err(
                CommandError::disabled(format!("element {} is locked", p.id.0)).with_param("id"),
            );
        }
        let mut new = e.clone();
        let ElementKind::Table(t) = &mut new.kind else {
            return Err(CommandError::bad_params(format!(
                "element {} is a {}, not a table",
                p.id.0,
                e.kind.type_name()
            ))
            .with_param("id"));
        };
        apply(t, p.edit)?;
        validate_element(&new)?;
        *e = new.clone();
        Ok(ElementOutput { element: new })
    }
}
