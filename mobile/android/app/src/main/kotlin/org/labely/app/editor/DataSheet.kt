package org.labely.app.editor

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.labely.app.R
import org.labely.app.engine.Serial
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.NumberField
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * The data source's options: which sheet of a spreadsheet is read and whether its first row
 * names the columns, an element for a column, one record per picture, and the serial numbers.
 */
@Composable
fun DataSheet(ed: EditorState, onPick: () -> Unit, onImages: () -> Unit, onDismiss: () -> Unit) {
    val data = ed.data
    var serial by remember { mutableStateOf<Serial?>(null) }
    var adding by remember { mutableStateOf(false) }
    val serials = ed.doc?.serials.orEmpty()
    Sheet(stringResource(R.string.tool_data), onDismiss) {
        if (data != null && ed.canReread) {
            Group {
                if (ed.sheets.size > 1) {
                    MenuRow(stringResource(R.string.data_sheet), ed.sheet.orEmpty(), ed.sheets.map { it to it }) { if (it != ed.sheet) ed.reread(sheet = it) }
                    RowDivider()
                }
                SwitchRow(stringResource(R.string.data_header), stringResource(R.string.data_header_sub), ed.header) { ed.reread(header = it) }
            }
        }
        if (data != null) {
            Heading(stringResource(R.string.data_add_column))
            Group {
                data.columns.forEachIndexed { i, column ->
                    if (i > 0) RowDivider()
                    ListRow(
                        column,
                        ed.value(column)?.ifEmpty { null },
                        onClick = {
                            ed.addBoundText(column)
                            onDismiss()
                        },
                        leading = { IconTile(Ic.Link, Palette.text2, Palette.bg) },
                    ) { LIcon(Ic.Plus, Palette.primary, 18.dp) }
                }
            }
        }

        Heading(stringResource(R.string.serials))
        Text(stringResource(R.string.serials_sub), style = Type.caption)
        Group {
            if (serials.isEmpty()) ListRow(stringResource(R.string.serials_none))
            serials.forEachIndexed { i, s ->
                if (i > 0) RowDivider()
                ListRow(
                    "{{${s.name}}}",
                    (0L..2L).joinToString(", ") { s.value(it) } + "…",
                    onClick = { serial = s },
                    leading = { IconTile(Ic.Hash) },
                ) {
                    IconButton(Ic.Plus, { ed.addBoundText(s.name); onDismiss() }, Palette.primary, size = 36.dp)
                    IconButton(Ic.Trash, { ed.removeSerial(s.name) }, Palette.red, size = 36.dp)
                }
            }
        }
        SecondaryButton(stringResource(R.string.serial_add), { adding = true }, Modifier.fillMaxWidth(), Ic.Plus)

        Heading(stringResource(R.string.data_source))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            SecondaryButton(stringResource(if (data == null) R.string.data_choose else R.string.data_choose_other), { onPick(); onDismiss() }, Modifier.weight(1f), Ic.Open)
            SecondaryButton(stringResource(R.string.data_images), { onImages(); onDismiss() }, Modifier.weight(1f), Ic.Image)
        }
        if (data != null) SecondaryButton(stringResource(R.string.data_remove), { ed.detachData(); onDismiss() }, Modifier.fillMaxWidth(), Ic.Trash)
    }
    val editing = serial
    if (editing != null || adding) {
        val taken = serials.map { it.name }
        // A new counter is `serial`, then `serial2`, `serial3`…
        val fresh = generateSequence(1) { it + 1 }.map { if (it == 1) "serial" else "serial$it" }.first { it !in taken }
        SerialSheet(
            ed,
            editing ?: Serial(fresh, 1, 1, 0, "", ""),
            onDismiss = {
                serial = null
                adding = false
            },
        )
    }
}

/** A serial counter: start, step, digits, prefix and suffix, with the first values it gives. */
@Composable
fun SerialSheet(ed: EditorState, initial: Serial, onDismiss: () -> Unit) {
    var s by remember { mutableStateOf(initial) }
    val focus = LocalFocusManager.current
    Sheet(stringResource(R.string.serial_title, initial.name), onDismiss) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            NumberField(stringResource(R.string.serial_start), s.start.toDouble(), { s = s.copy(start = it.toLong()) }, Modifier.weight(1f), "")
            NumberField(stringResource(R.string.serial_step), s.step.toDouble(), { s = s.copy(step = it.toLong()) }, Modifier.weight(1f), "")
            NumberField(stringResource(R.string.serial_digits), s.digits.toDouble(), { s = s.copy(digits = it.toInt().coerceIn(0, 18)) }, Modifier.weight(1f), "")
        }
        Heading(stringResource(R.string.serial_prefix))
        ContentField(s.prefix, "", live = true) { s = s.copy(prefix = it) }
        Heading(stringResource(R.string.serial_suffix))
        ContentField(s.suffix, "", live = true) { s = s.copy(suffix = it) }
        Text(stringResource(R.string.serial_sample, (0L..2L).joinToString(", ") { s.value(it) }), style = Type.bodyStrong)
        PrimaryButton(
            stringResource(R.string.save),
            {
                focus.clearFocus()
                ed.setSerial(s.name, s.start, s.step, s.digits, s.prefix, s.suffix)
                onDismiss()
            },
            Modifier.fillMaxWidth(),
        )
    }
}
