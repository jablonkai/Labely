package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONObject
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.engine.objects
import org.labely.app.ui.NumberField
import org.labely.app.ui.Stepper
import org.labely.app.ui.mm
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** A table element's grid as the editor needs it. */
class TableModel(e: Element) {
    val columnWidths: List<Double> = e.raw.optJSONArray("column_widths").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optDouble(it, 1.0) } }
    val rowHeights: List<Double> = e.raw.optJSONArray("row_heights").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optDouble(it, 1.0) } }
    val cells: List<List<JSONObject>> = e.raw.optJSONArray("cells").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optJSONArray(it).objects() } }
    val merges: List<CellRange> = e.raw.optJSONArray("merges").objects().map { CellRange(it.optInt("row"), it.optInt("col"), it.optInt("rows", 1), it.optInt("cols", 1)) }
    val rows: Int get() = rowHeights.size
    val columns: Int get() = columnWidths.size

    fun cell(row: Int, col: Int): JSONObject = cells.getOrNull(row)?.getOrNull(col) ?: JSONObject()

    /** The block ([row], [col]) belongs to: its merge, or itself. */
    fun block(row: Int, col: Int): CellRange = merges.firstOrNull { it.contains(row, col) } ?: CellRange(row, col, 1, 1)
}

/**
 * The cell editor of table [id] (`element.table_edit`): pick a cell in the grid, then change
 * its text and style, add or remove its row and column, merge it with its neighbour or split
 * it, and set the row's height and the column's width.
 */
@Composable
fun TableSheet(ed: EditorState, id: Long, onDismiss: () -> Unit) {
    val e = ed.doc?.elements?.firstOrNull { it.id == id && it.type == "table" }
    if (e == null) {
        LaunchedEffect(id) { onDismiss() }
        return
    }
    val t = remember(e) { TableModel(e) }
    var row by remember { mutableIntStateOf(0) }
    var col by remember { mutableIntStateOf(0) }
    // The selection is always the top-left cell of a block, and inside the table.
    val block = t.block(row.coerceIn(0, (t.rows - 1).coerceAtLeast(0)), col.coerceIn(0, (t.columns - 1).coerceAtLeast(0)))
    val r = block.row
    val c = block.col
    val cell = t.cell(r, c)
    fun edit(op: String, vararg params: Pair<String, Any?>) = ed.tableEdit(id, op, *params)
    fun setCell(vararg params: Pair<String, Any?>) = edit("set_cell", "row" to r, "col" to c, *params)

    Sheet(stringResource(R.string.kind_table), onDismiss) {
        Column(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            for (y in 0 until t.rows) {
                Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    for (x in 0 until t.columns) {
                        val b = t.block(y, x)
                        val origin = b.row == y && b.col == x
                        val on = b.row == r && b.col == c
                        val data = t.cell(b.row, b.col)
                        Box(
                            Modifier.width(72.dp).height(40.dp)
                                .background(if (on) Palette.primaryTint else if (data.optBoolean("invert")) Palette.text else if (origin) Palette.surface else Palette.bg)
                                .border(if (on) 2.dp else 1.dp, if (on) Palette.primary else Palette.border)
                                .clickable {
                                    row = b.row
                                    col = b.col
                                }
                                .padding(4.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (origin) {
                                Text(
                                    data.optString("text"),
                                    style = Type.caption.copy(
                                        color = if (data.optBoolean("invert") && !on) Palette.surface else Palette.text,
                                        fontWeight = if (data.optBoolean("bold")) FontWeight.Bold else FontWeight.Normal,
                                        fontSize = 12.sp,
                                    ),
                                    maxLines = 2,
                                    overflow = TextOverflow.Ellipsis,
                                    textAlign = TextAlign.Center,
                                )
                            }
                        }
                    }
                }
            }
        }

        Heading(stringResource(R.string.table_cell, r + 1, c + 1))
        ContentField(cell.optString("text"), stringResource(R.string.text_hint), lines = 3) { setCell("text" to it) }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val size = if (cell.has("font_size_pt") && !cell.isNull("font_size_pt")) cell.optDouble("font_size_pt") else e.fontSizePt
            Stepper(mm(size), { setCell("font_size_pt" to (size - 1).coerceAtLeast(4.0)) }, { setCell("font_size_pt" to (size + 1).coerceAtMost(200.0)) })
            Box(Modifier.weight(1f))
            Toggle(Ic.Bold, cell.optBoolean("bold")) { setCell("bold" to !cell.optBoolean("bold")) }
            Toggle(Ic.Contrast, cell.optBoolean("invert")) { setCell("invert" to !cell.optBoolean("invert")) }
            val align = cell.optString("align").ifEmpty { e.raw.optString("align", "center") }
            Toggle(Ic.TLeft, align == "left") { setCell("align" to "left") }
            Toggle(Ic.TCenter, align == "center") { setCell("align" to "center") }
            Toggle(Ic.TRight, align == "right") { setCell("align" to "right") }
        }

        Heading(stringResource(R.string.table_rows))
        Actions {
            Action(Ic.Up, stringResource(R.string.table_row_above), t.rows < MAX_TABLE) { edit("insert_row", "at" to r) }
            Action(Ic.Down, stringResource(R.string.table_row_below), t.rows < MAX_TABLE) { edit("insert_row", "at" to r + block.rows) }
            Action(Ic.Trash, stringResource(R.string.table_row_delete), t.rows > 1, tint = Palette.red) { edit("delete_row", "at" to r) }
        }
        Heading(stringResource(R.string.table_columns))
        Actions {
            Action(Ic.Left, stringResource(R.string.table_column_left), t.columns < MAX_TABLE) { edit("insert_column", "at" to c) }
            Action(Ic.Right, stringResource(R.string.table_column_right), t.columns < MAX_TABLE) { edit("insert_column", "at" to c + block.cols) }
            Action(Ic.Trash, stringResource(R.string.table_column_delete), t.columns > 1, tint = Palette.red) { edit("delete_column", "at" to c) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // Heights and widths are shares of the table: 1 and 2 make a third and two thirds.
            NumberField(stringResource(R.string.table_row_height), t.rowHeights.getOrElse(r) { 1.0 }, { if (it > 0) edit("resize_row", "at" to r, "height" to it) }, Modifier.weight(1f), "")
            NumberField(stringResource(R.string.table_column_width), t.columnWidths.getOrElse(c) { 1.0 }, { if (it > 0) edit("resize_column", "at" to c, "width" to it) }, Modifier.weight(1f), "")
        }
        Text(stringResource(R.string.table_size_hint), style = Type.caption)

        Heading(stringResource(R.string.table_merge))
        Actions {
            Action(Ic.Right, stringResource(R.string.table_merge_right), c + block.cols < t.columns) { edit("merge", "row" to r, "col" to c, "rows" to block.rows, "cols" to block.cols + 1) }
            Action(Ic.Down, stringResource(R.string.table_merge_down), r + block.rows < t.rows) { edit("merge", "row" to r, "col" to c, "rows" to block.rows + 1, "cols" to block.cols) }
            Action(Ic.Table, stringResource(R.string.table_split), block.rows * block.cols > 1) { edit("split", "row" to r, "col" to c) }
            Action(Ic.Close, stringResource(R.string.table_clear), tint = Palette.red) { edit("clear") }
        }
    }
}

/** Most rows or columns the engine allows in a table. */
private const val MAX_TABLE = 64
