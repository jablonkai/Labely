package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.PRESET_SIZES
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.NumberField
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.Segmented
import org.labely.app.ui.SheetHeader
import org.labely.app.ui.mm
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Bottom sheet with the design's header; content scrolls when it is tall. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Sheet(title: String, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = Palette.surface,
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding()) {
            SheetHeader(title, onDismiss)
            Column(
                Modifier.verticalScroll(rememberScrollState()).padding(bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
                content = content,
            )
        }
    }
}

@Composable
fun InsertSheet(ed: EditorState, onImage: () -> Unit, onDismiss: () -> Unit) {
    val text = stringResource(R.string.new_text)
    val items = listOf<Triple<Ic, String, () -> Unit>>(
        Triple(Ic.Text, stringResource(R.string.kind_text)) { ed.addText(text) },
        Triple(Ic.Barcode, stringResource(R.string.kind_barcode)) { ed.addBarcode() },
        Triple(Ic.Qr, stringResource(R.string.kind_code2d)) { ed.addQr() },
        Triple(Ic.Rect, stringResource(R.string.kind_rect)) { ed.addRect() },
        Triple(Ic.Ellipse, stringResource(R.string.kind_ellipse)) { ed.addEllipse() },
        Triple(Ic.Line, stringResource(R.string.kind_line)) { ed.addLine() },
        Triple(Ic.Image, stringResource(R.string.kind_image), onImage),
    )
    Sheet(stringResource(R.string.tool_insert), onDismiss) {
        items.chunked(3).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { (icon, label, add) ->
                    Column(
                        Modifier.weight(1f).aspectRatio(1.2f).clip(RoundedCornerShape(16.dp)).background(Palette.bg)
                            .clickable { add(); onDismiss() }.padding(12.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically),
                    ) {
                        LIcon(icon, Palette.primary, 28.dp)
                        Text(label, style = Type.bodyStrong.copy(fontSize = 14.sp))
                    }
                }
            }
        }
    }
}

/** M5 · Rétegek: elements top-first; tap selects, arrows reorder. */
@Composable
fun LayersSheet(ed: EditorState, onDismiss: () -> Unit) {
    val elements = ed.doc?.elements.orEmpty().reversed()
    Sheet(stringResource(R.string.tool_layers), onDismiss) {
        Group {
            LazyColumn(Modifier.heightIn(max = 420.dp)) {
                items(elements, key = { it.id }) { e ->
                    if (e != elements.first()) RowDivider()
                    ListRow(
                        title = elementTitle(e),
                        subtitle = kindName(e.type),
                        onClick = { ed.selectedId = e.id; onDismiss() },
                        leading = {
                            if (e.id == ed.selectedId) IconTile(kindIcon(e.type)) else IconTile(kindIcon(e.type), Palette.text2, Palette.bg)
                        },
                    ) {
                        IconButton(Ic.Up, { ed.exec("element.reorder", "id" to e.id, "to" to "forward") }, enabled = e != elements.first())
                        IconButton(Ic.Down, { ed.exec("element.reorder", "id" to e.id, "to" to "backward") }, enabled = e != elements.last())
                    }
                }
            }
            if (elements.isEmpty()) ListRow(stringResource(R.string.layers_empty))
        }
    }
}

/**
 * M11 · Új címke: size presets, custom size, paper type and orientation. Used for a new label
 * and, with another title, for changing the open one.
 */
@Composable
fun LabelSheet(
    title: String,
    confirm: String,
    initial: Triple<Double, Double, String>,
    onDismiss: () -> Unit,
    onConfirm: (Double, Double, String) -> Unit,
) {
    var w by remember { mutableDoubleStateOf(initial.first) }
    var h by remember { mutableDoubleStateOf(initial.second) }
    var media by remember { mutableStateOf(initial.third) }
    var mediaMenu by remember { mutableStateOf(false) }
    Sheet(title, onDismiss) {
        Box(Modifier.fillMaxWidth().height(160.dp).clip(RoundedCornerShape(16.dp)).background(Palette.canvas), contentAlignment = Alignment.Center) {
            val scale = minOf(200.0 / w, 100.0 / h)
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text("${mm(w)} mm", style = Type.small)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box(Modifier.size((w * scale).dp, (h * scale).dp).clip(RoundedCornerShape(6.dp)).background(Palette.surface))
                    Text("${mm(h)} mm", style = Type.small)
                }
            }
        }
        Text(stringResource(R.string.size), style = Type.bodyStrong.copy(color = Palette.text2))
        PRESET_SIZES.chunked(3).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                row.forEach { (pw, ph) ->
                    val on = (pw == w && ph == h) || (pw == h && ph == w)
                    val shape = RoundedCornerShape(12.dp)
                    Text(
                        sizeText(pw, ph),
                        style = Type.bodyStrong.copy(color = if (on) Palette.primaryDark else Palette.text, fontSize = 14.sp),
                        modifier = Modifier.weight(1f).clip(shape).background(if (on) Palette.primaryTint else Palette.surface)
                            .border(if (on) 2.dp else 1.dp, if (on) Palette.primary else Palette.border, shape)
                            .clickable { if (w < h) { w = ph; h = pw } else { w = pw; h = ph } }
                            .padding(vertical = 12.dp),
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            NumberField(stringResource(R.string.width), w, { if (it > 0) w = it }, Modifier.weight(1f))
            NumberField(stringResource(R.string.height), h, { if (it > 0) h = it }, Modifier.weight(1f))
        }
        Group {
            Box {
                ListRow(stringResource(R.string.paper_type), onClick = { mediaMenu = true }) {
                    Text(mediaName(media), style = Type.body.copy(color = Palette.muted))
                    LIcon(Ic.ChevRight, Palette.muted, 18.dp)
                }
                DropdownMenu(mediaMenu, { mediaMenu = false }) {
                    MEDIA.forEach { m -> DropdownMenuItem({ Text(mediaName(m)) }, { media = m; mediaMenu = false }) }
                }
            }
            RowDivider()
            ListRow(stringResource(R.string.orientation)) {
                Segmented(
                    listOf(stringResource(R.string.landscape), stringResource(R.string.portrait)),
                    if (w >= h) 0 else 1,
                ) { i -> if ((i == 0) != (w >= h)) { val t = w; w = h; h = t } }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SecondaryButton(stringResource(R.string.cancel), onDismiss, Modifier.weight(1f).height(52.dp))
            PrimaryButton(confirm, { onConfirm(w, h, media) }, Modifier.weight(1f))
        }
    }
}

@Composable
fun SaveTemplateDialog(initial: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var name by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.save_template)) },
        text = { OutlinedTextField(name, { name = it }, singleLine = true, label = { Text(stringResource(R.string.template_name)) }) },
        confirmButton = { TextButton({ onSave(name.trim()) }, enabled = name.isNotBlank()) { Text(stringResource(R.string.save)) } },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
