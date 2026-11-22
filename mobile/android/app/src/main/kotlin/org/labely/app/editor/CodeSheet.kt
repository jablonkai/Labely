package org.labely.app.editor

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.LSwitch
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.Segmented
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * M6 · Vonalkód: symbology, value with its validity, the human-readable text and the binding
 * of barcode or 2D code [id]. A value the engine cannot encode is shown in red and not applied.
 */
@Composable
fun CodeSheet(ed: EditorState, id: Long, onBinding: () -> Unit, onDismiss: () -> Unit) {
    val e = ed.doc?.elements?.firstOrNull { it.id == id && (it.type == "barcode" || it.type == "code2d") }
    if (e == null) {
        LaunchedEffect(id) { onDismiss() }
        return
    }
    val element by rememberUpdatedState(e)
    val options = if (e.type == "barcode") SYMBOLOGIES_1D else SYMBOLOGIES_2D
    // What the sheet shows; it differs from the element while the engine rejects it.
    var symbology by remember(e.symbology) { mutableStateOf(e.symbology) }
    var value by remember(e.data) { mutableStateOf(e.data.orEmpty()) }
    var check by remember { mutableStateOf<CodeCheck?>(null) }
    val apply = { s: String, v: String ->
        symbology = s
        value = v
        ed.app.scope.launch { check = ed.applyCode(element, s, v) }
    }
    // Changes nothing; tells whether the value as it is encodes.
    LaunchedEffect(id) { check = ed.applyCode(element, symbology, value) }
    val bound = ed.boundColumn(e)
    var scanning by remember { mutableStateOf(false) }
    if (scanning) {
        // A scanned code of the element's kind brings its symbology; otherwise only its value.
        ScanSheet({ scanning = false }) { code ->
            scanning = false
            apply(if (code.kind == e.type) code.symbology else symbology, code.text)
        }
    }

    Sheet(kindName(e.type), onDismiss) {
        Segmented(options.map { it.second }, options.indexOfFirst { it.first == symbology }, Modifier.fillMaxWidth(), scroll = true) { apply(options[it].first, value) }

        Text(stringResource(R.string.code_value), style = Type.bodyStrong.copy(color = Palette.text2))
        ContentField(
            value,
            stringResource(R.string.data_hint),
            trailing = {
                Row {
                    IconButton(Ic.Scan, { scanning = true }, Palette.muted)
                    if (ed.data != null || bound != null) IconButton(Ic.Link, onBinding, if (bound != null) Palette.primary else Palette.muted)
                }
            },
        ) { apply(symbology, it) }
        val c = check
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            when {
                c == null -> Text(" ", style = Type.body)
                c.error != null -> {
                    LIcon(Ic.Warn, Palette.red, 18.dp)
                    Text(c.error, style = Type.body.copy(color = Palette.red))
                }
                bound != null -> {
                    LIcon(Ic.Link, Palette.primary, 18.dp)
                    Text(stringResource(R.string.code_bound, bound), style = Type.body.copy(color = Palette.primaryDark))
                }
                else -> {
                    LIcon(Ic.Check, Palette.green, 18.dp)
                    Text(
                        if (c.checkDigit != null) stringResource(R.string.code_valid_check, c.checkDigit) else stringResource(R.string.code_valid),
                        style = Type.body.copy(color = Palette.green),
                    )
                }
            }
        }

        val barcode = e.type == "barcode"
        val readable = e.humanReadable != "none"
        if (barcode || ed.data != null || bound != null) {
            Group {
                if (barcode) {
                    ListRow(
                        stringResource(R.string.code_hr),
                        stringResource(
                            when (e.humanReadable) {
                                "none" -> R.string.code_hr_off
                                "above" -> R.string.code_hr_above_sub
                                else -> R.string.code_hr_below_sub
                            },
                        ),
                    ) { LSwitch(readable) { ed.setProps(e.id, "human_readable" to if (it) "below" else "none") } }
                }
                if (ed.data != null || bound != null) {
                    if (barcode) RowDivider()
                    ListRow(
                        stringResource(R.string.binding),
                        when {
                            bound == null -> stringResource(R.string.binding_none)
                            ed.sourceName != null -> stringResource(R.string.binding_column_source, bound, ed.sourceName.orEmpty())
                            else -> stringResource(R.string.binding_column, bound)
                        },
                        onClick = onBinding,
                        leading = { IconTile(Ic.Link) },
                    ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
                }
                if (barcode && readable) {
                    RowDivider()
                    ListRow(stringResource(R.string.code_hr_position)) {
                        Segmented(
                            listOf(stringResource(R.string.code_hr_below), stringResource(R.string.code_hr_above)),
                            if (e.humanReadable == "above") 1 else 0,
                        ) { ed.setProps(e.id, "human_readable" to if (it == 1) "above" else "below") }
                    }
                }
            }
        }
        // The readable text's size (barcode) and the error correction (QR).
        CodeExtras(ed, e)
    }
}
