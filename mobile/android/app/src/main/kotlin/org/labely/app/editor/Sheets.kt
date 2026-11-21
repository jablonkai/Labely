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
import androidx.compose.foundation.horizontalScroll
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import org.labely.app.PRESET_SIZES
import org.labely.app.R
import org.labely.app.engine.Detected
import org.labely.app.ui.Chip
import org.labely.app.ui.LengthField
import org.labely.app.ui.SearchField
import org.labely.app.ui.lengthText
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

/** Everything that can be put on the label: the element kinds, computed texts and a code read from a picture. */
@Composable
fun InsertSheet(ed: EditorState, onImage: () -> Unit, onTable: () -> Unit, onIcon: () -> Unit, onBorder: () -> Unit, onCodeImage: () -> Unit, onDismiss: () -> Unit) {
    val text = stringResource(R.string.new_text)
    // The second value tells whether the sheet closes: pickers that follow replace it themselves.
    val items = listOf<Triple<Ic, String, () -> Unit>>(
        Triple(Ic.Text, stringResource(R.string.kind_text)) { ed.addText(text); onDismiss() },
        Triple(Ic.Barcode, stringResource(R.string.kind_barcode)) { ed.addBarcode(); onDismiss() },
        Triple(Ic.Qr, stringResource(R.string.kind_code2d)) { ed.addQr(); onDismiss() },
        Triple(Ic.Image, stringResource(R.string.kind_image)) { onImage(); onDismiss() },
        Triple(Ic.Table, stringResource(R.string.kind_table), onTable),
        Triple(Ic.Star, stringResource(R.string.kind_icon), onIcon),
        Triple(Ic.Rect, stringResource(R.string.kind_rect)) { ed.addRect(); onDismiss() },
        Triple(Ic.Ellipse, stringResource(R.string.kind_ellipse)) { ed.addEllipse(); onDismiss() },
        Triple(Ic.Line, stringResource(R.string.kind_line)) { ed.addLine(); onDismiss() },
        Triple(Ic.Border, stringResource(R.string.kind_border), onBorder),
        Triple(Ic.Calendar, stringResource(R.string.insert_date)) { ed.addBound(EditorState.DATE_BINDING); onDismiss() },
        Triple(Ic.Clock, stringResource(R.string.insert_time)) { ed.addBound(EditorState.TIME_BINDING); onDismiss() },
        Triple(Ic.Hash, stringResource(R.string.insert_serial)) { ed.addBound(EditorState.SERIAL_BINDING); onDismiss() },
        Triple(Ic.Scan, stringResource(R.string.insert_code_from_image)) { onCodeImage(); onDismiss() },
    )
    Sheet(stringResource(R.string.tool_insert), onDismiss) {
        items.chunked(3).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { (icon, label, add) -> InsertTile(icon, label, Modifier.weight(1f), add) }
                repeat(3 - row.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/**
 * M5 · Rétegek: elements top-first, with search. A tap selects (with "Select several" it adds
 * or removes); the lock and the eye lock and hide; the arrows restack the selection.
 */
@Composable
fun LayersSheet(ed: EditorState, onDismiss: () -> Unit) {
    val all = ed.doc?.elements.orEmpty().reversed()
    var query by remember { mutableStateOf("") }
    val names = all.associate { it.id to elementTitle(it) }
    val elements = all.filter { query.isBlank() || names[it.id].orEmpty().contains(query.trim(), ignoreCase = true) || kindName(it.type).contains(query.trim(), ignoreCase = true) }
    val picked = ed.selection.isNotEmpty()
    Sheet(stringResource(R.string.tool_layers), onDismiss) {
        SearchField(query, stringResource(R.string.layers_search)) { query = it }
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Chip(stringResource(R.string.select_several), ed.multiSelect) { if (ed.multiSelect) ed.select(emptyList()) else ed.multiSelect = true }
            Chip(stringResource(R.string.select_all), false) { ed.selectAll() }
            if (picked) Chip(stringResource(R.string.select_none), false) { ed.select(emptyList()) }
        }
        Actions {
            Action(Ic.Front, stringResource(R.string.order_front), picked) { ed.reorder("front") }
            Action(Ic.Up, stringResource(R.string.order_forward), picked) { ed.reorder("forward") }
            Action(Ic.Down, stringResource(R.string.order_backward), picked) { ed.reorder("backward") }
            Action(Ic.Back, stringResource(R.string.order_back), picked) { ed.reorder("back") }
        }
        Group {
            LazyColumn(Modifier.heightIn(max = 380.dp)) {
                items(elements, key = { it.id }) { e ->
                    if (e != elements.first()) RowDivider()
                    val on = e.id in ed.selection
                    ListRow(
                        title = names[e.id].orEmpty(),
                        subtitle = listOfNotNull(kindName(e.type), stringResource(R.string.grouped).takeIf { e.group != null }, stringResource(R.string.mirrored).takeIf { e.mirrorOf != null }).joinToString(" · "),
                        onClick = {
                            if (ed.multiSelect) {
                                ed.toggle(e.id)
                            } else {
                                ed.selectedId = e.id
                                onDismiss()
                            }
                        },
                        leading = { if (on) IconTile(kindIcon(e.type)) else IconTile(kindIcon(e.type), Palette.text2, Palette.bg) },
                    ) {
                        IconButton(if (e.locked) Ic.Lock else Ic.Unlock, { ed.lock(listOf(e.id), !e.locked) }, if (e.locked) Palette.primary else Palette.muted, size = 36.dp)
                        IconButton(if (e.hidden) Ic.EyeOff else Ic.Eye, { ed.setHidden(listOf(e.id), !e.hidden) }, if (e.hidden) Palette.primary else Palette.muted, size = 36.dp)
                    }
                }
            }
            if (all.isEmpty()) ListRow(stringResource(R.string.layers_empty)) else if (elements.isEmpty()) ListRow(stringResource(R.string.layers_no_match))
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
    /** Reads the roll in the connected printer; null when no printer is connected. */
    detect: (suspend () -> Detected?)? = null,
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
                Text(lengthText(w), style = Type.small)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box(Modifier.size((w * scale).dp, (h * scale).dp).clip(RoundedCornerShape(6.dp)).background(Palette.surface))
                    Text(lengthText(h), style = Type.small)
                }
            }
        }
        if (detect != null) {
            // "Read the loaded paper" and "Use this size", as in the studio's New label dialog.
            val scope = rememberCoroutineScope()
            var reading by remember { mutableStateOf(false) }
            var found by remember { mutableStateOf<Detected?>(null) }
            var read by remember { mutableStateOf(false) }
            SecondaryButton(
                stringResource(if (reading) R.string.paper_reading else R.string.paper_read_loaded),
                {
                    reading = true
                    scope.launch {
                        found = detect()
                        read = true
                        reading = false
                    }
                },
                Modifier.fillMaxWidth(),
                Ic.Scan,
                enabled = !reading,
            )
            val paper = found?.paper
            when {
                !read -> Unit
                paper != null -> Group {
                    ListRow(paper.name, sizeText(paper.widthMm, paper.heightMm) + " · " + mediaName(paper.media), leading = { IconTile(Ic.Label, Palette.green, Palette.greenTint) }) {
                        Text(
                            stringResource(R.string.paper_use_size),
                            style = Type.bodyStrong.copy(color = Palette.primary),
                            modifier = Modifier.clickable {
                                w = paper.widthMm
                                h = paper.heightMm
                                media = paper.media
                            },
                        )
                    }
                }
                found?.rfid == true -> Text(stringResource(R.string.paper_unknown_roll, found?.barcode.orEmpty()), style = Type.caption)
                else -> Text(stringResource(R.string.paper_no_tag), style = Type.caption)
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
            LengthField(stringResource(R.string.width), w, { if (it > 0) w = it }, Modifier.weight(1f))
            LengthField(stringResource(R.string.height), h, { if (it > 0) h = it }, Modifier.weight(1f))
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

/**
 * Name and folder of a template to save. When the library already has that template, Save
 * first asks and then replaces it.
 */
@Composable
fun SaveTemplateDialog(initial: String, folders: List<String>, exists: (String, String) -> Boolean, onDismiss: () -> Unit, onSave: (String, String) -> Unit) {
    var name by remember { mutableStateOf(initial) }
    var folder by remember { mutableStateOf("") }
    var newFolder by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var asked by remember { mutableStateOf(false) }
    val taken = exists(name.trim(), folder.trim())
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.save_template)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it; asked = false }, singleLine = true, label = { Text(stringResource(R.string.template_name)) })
                if (newFolder) {
                    OutlinedTextField(folder, { folder = it; asked = false }, singleLine = true, label = { Text(stringResource(R.string.folder_name)) })
                } else {
                    Box {
                        OutlinedTextField(
                            folder.ifEmpty { stringResource(R.string.folder_default) }, {}, readOnly = true, singleLine = true, label = { Text(stringResource(R.string.folder)) },
                            trailingIcon = { IconButton(Ic.ChevDown, { menu = true }, size = 36.dp) },
                        )
                        DropdownMenu(menu, { menu = false }) {
                            DropdownMenuItem({ Text(stringResource(R.string.folder_default)) }, { folder = ""; menu = false; asked = false })
                            folders.forEach { f -> DropdownMenuItem({ Text(f) }, { folder = f; menu = false; asked = false }) }
                            DropdownMenuItem({ Text(stringResource(R.string.folder_new)) }, { folder = ""; newFolder = true; menu = false })
                        }
                    }
                }
                if (taken && asked) Text(stringResource(R.string.template_replace_body, name.trim()), style = Type.body.copy(color = Palette.amber))
            }
        },
        confirmButton = {
            TextButton({ if (taken && !asked) asked = true else onSave(name.trim(), folder.trim()) }, enabled = name.isNotBlank()) {
                Text(stringResource(if (taken && asked) R.string.replace else R.string.save), color = if (taken && asked) Palette.red else Palette.primary)
            }
        },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
