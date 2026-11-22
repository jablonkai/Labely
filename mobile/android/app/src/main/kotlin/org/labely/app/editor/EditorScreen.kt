package org.labely.app.editor

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.labely.app.R
import org.labely.app.engine.Element
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.res.pluralStringResource
import org.labely.app.ui.IconButton
import org.labely.app.ui.LengthField
import org.labely.app.ui.jobs.JobsSheet
import org.labely.app.ui.mm
import org.labely.app.ui.IconTile
import org.labely.app.ui.NumberField
import org.labely.app.ui.Stepper
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

private enum class Sheet { Insert, Layers, Label, Paper, Check, SaveTemplate, Print, Export, Binding, Element, Icons, ChangeIcon, Border, NewTable, Data, Jobs }

/** M2 · Szerkesztő: canvas, inspector of the selection and the editing toolbar. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EditorScreen(ed: EditorState, onClose: () -> Unit) {
    var sheet by remember { mutableStateOf<Sheet?>(null) }
    // Text and number fields commit when they lose focus. Hiding the keyboard (Back) or opening
    // a sheet such as Print leaves focus in place, so clear it there: a pending edit is never lost.
    val focus = LocalFocusManager.current
    val imeVisible = WindowInsets.isImeVisible
    LaunchedEffect(imeVisible) { if (!imeVisible) focus.clearFocus() }
    val open = { s: Sheet ->
        focus.clearFocus()
        sheet = s
    }
    // M4: the data panel takes the inspector's place; selecting an element brings the inspector back.
    var dataPanel by remember { mutableStateOf(false) }
    LaunchedEffect(ed.selection) { if (ed.selection.isNotEmpty()) dataPanel = false }
    BackHandler {
        when {
            dataPanel -> dataPanel = false
            ed.selection.isNotEmpty() || ed.multiSelect -> ed.select(emptyList())
            else -> onClose()
        }
    }
    val doc = ed.doc ?: return

    val context = LocalContext.current.applicationContext
    val app = ed.app
    // "Print again" from the history opens the document with the print sheet.
    LaunchedEffect(Unit) {
        if (app.printOnOpen) {
            app.printOnOpen = false
            sheet = Sheet.Print
        }
        if (app.dataOnOpen) {
            app.dataOnOpen = false
            dataPanel = true
        }
    }
    // The loaded roll is compared with the label when a printer is there and when the label changes.
    LaunchedEffect(app.printer?.address, doc.label.widthMm, doc.label.heightMm, doc.label.media) {
        if (app.printer != null) ed.detectPaper()
    }
    var found by remember { mutableStateOf<List<FoundCode>?>(null) }
    // The data panel's camera scanner: the scanned code becomes an element.
    var scanning by remember { mutableStateOf(false) }
    val noCode = stringResource(R.string.found_codes_none)
    val notFont = stringResource(R.string.font_not_added)
    val copySaved = stringResource(R.string.save_copy_done)

    val pickData = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            app.scope.launch {
                app.report {
                    val (name, bytes) = readPicked(context, uri)
                    ed.attachDataNow(name, bytes)
                }
            }
        }
    }
    val pickImage = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) {
            app.scope.launch {
                app.report {
                    val image = loadImage(context, uri)
                    ed.addImage(image.bytes, image.mime, image.width, image.height)
                }
            }
        }
    }
    // "Code from image": the barcodes and 2D codes of a photo become elements.
    val pickCode = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) {
            app.scope.launch {
                app.report {
                    val codes = ed.decode(loadImage(context, uri).bytes)
                    when (codes.size) {
                        0 -> app.message = noCode
                        1 -> ed.addCode(codes[0])
                        else -> found = codes
                    }
                }
            }
        }
    }
    // One record per picture.
    val pickImages = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(MAX_DATA_IMAGES)) { uris ->
        if (uris.isNotEmpty()) {
            app.scope.launch {
                app.report {
                    val pictures = uris.mapIndexed { i, uri ->
                        val image = loadImage(context, uri)
                        "image-${i + 1}.${if (image.mime == "image/png") "png" else "jpg"}" to image.bytes
                    }
                    ed.attachImages(pictures).join()
                }
            }
        }
    }
    val pickFont = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            app.scope.launch {
                app.report {
                    val (name, bytes) = readPicked(context, uri)
                    val added = app.addFont(name, bytes)
                    val e = ed.selected
                    if (added.isEmpty()) app.message = notFont else if (e != null && (e.type == "text" || e.type == "table")) ed.setProps(e.id, "font_family" to added[0])
                }
            }
        }
    }
    // "Save a copy": the .labely file goes where the user chooses.
    val saveCopy = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        if (uri != null) {
            app.scope.launch {
                app.report {
                    val bytes = ed.fileBytes()
                    writeTo(context, bytes, uri)
                    app.message = copySaved
                }
            }
        }
    }
    val visual = PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)

    Column(Modifier.fillMaxSize().background(Palette.surface).statusBarsPadding().navigationBarsPadding()) {
        TopBar(
            ed, onClose,
            onPrint = { open(Sheet.Print) },
            onExport = { open(Sheet.Export) },
            onCheck = { open(Sheet.Check) },
            onSaveCopy = { saveCopy.launch(exportBaseName(doc.name) + ".labely") },
            onJobs = { open(Sheet.Jobs) },
        )
        Box(Modifier.weight(1f).fillMaxWidth().background(Palette.canvas)) {
            LabelCanvas(ed, Modifier.fillMaxSize().padding(bottom = 56.dp))
            Column(Modifier.align(Alignment.BottomCenter).padding(start = 12.dp, end = 12.dp, bottom = 16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                // Loaded, mismatch or unknown roll, as the studio's paper banner.
                if (app.detected?.let { it.rfid && (it.paper == null || it.matchesLabel == false) } == true) {
                    PaperBanner(app.detected, onUse = { ed.applyPaper(it.id) }, onLink = { open(Sheet.Paper) })
                }
                PaperBar(sizeText(doc.label.widthMm, doc.label.heightMm) + " · " + mediaName(doc.label.media)) { open(Sheet.Paper) }
            }
        }
        HorizontalDivider(color = Palette.border)
        if (dataPanel) {
            DataPanel(ed, Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT), onImages = { pickImages.launch(visual) }, onOptions = { open(Sheet.Data) }, onScan = { scanning = true }) { pickData.launch(DATA_MIME_TYPES) }
        } else {
            Inspector(ed, onBinding = { open(Sheet.Binding) }, onMore = { open(Sheet.Element) }, onIcon = { open(Sheet.ChangeIcon) })
        }
        HorizontalDivider(color = Palette.border)
        Toolbar(
            data = dataPanel,
            onInsert = { open(Sheet.Insert) },
            onLayers = { open(Sheet.Layers) },
            onData = {
                focus.clearFocus()
                dataPanel = !dataPanel
            },
            onTemplate = { open(Sheet.SaveTemplate) },
            onLabel = { open(Sheet.Label) },
        )
    }

    val close = { sheet = null }
    // The code and table sheets are under the pickers they can open.
    ed.codeSheet?.let { id -> CodeSheet(ed, id, onBinding = { sheet = Sheet.Binding }) { ed.codeSheet = null } }
    ed.tableSheet?.let { id -> TableSheet(ed, id) { ed.tableSheet = null } }
    found?.let { codes -> FoundCodesSheet(ed, codes) { found = null } }
    if (scanning) {
        ScanSheet({ scanning = false }) { code ->
            scanning = false
            ed.addCode(code)
        }
    }
    when (sheet) {
        Sheet.Insert -> InsertSheet(
            ed,
            onImage = { pickImage.launch(visual) },
            onTable = { sheet = Sheet.NewTable },
            onIcon = { sheet = Sheet.Icons },
            onBorder = { sheet = Sheet.Border },
            onCodeImage = { pickCode.launch(visual) },
            onDismiss = close,
        )
        Sheet.Layers -> LayersSheet(ed, close)
        Sheet.Label -> LabelSettingsSheet(ed, onPaper = { sheet = Sheet.Paper }, onCheck = { sheet = Sheet.Check }, onDismiss = close)
        Sheet.Paper -> PaperSheet(ed, close)
        Sheet.Check -> CheckSheet(ed, close)
        Sheet.SaveTemplate -> SaveTemplateDialog(doc.name, app.folders, app::templateExists, close) { name, folder ->
            ed.saveTemplate(name, folder)
            close()
        }
        Sheet.Print -> PrintSheet(ed, close)
        Sheet.Export -> ExportSheet(ed, close)
        Sheet.Jobs -> JobsSheet(app, close)
        Sheet.Binding -> {
            val e = ed.doc?.elements?.firstOrNull { it.id == (ed.codeSheet ?: ed.selectedId) }
            if (e != null) BindingSheet(ed, e, close) else LaunchedEffect(Unit) { close() }
        }
        Sheet.Element -> ElementSheet(ed, onBinding = { sheet = Sheet.Binding }, onPickIcon = { sheet = Sheet.ChangeIcon }, onAddFont = { pickFont.launch(arrayOf("*/*")) }, onDismiss = close)
        Sheet.Icons -> IconSheet(ed, close) { ed.addIcon(it) }
        Sheet.ChangeIcon -> IconSheet(ed, close) { name -> ed.selected?.takeIf { it.type == "icon" }?.let { ed.setProps(it.id, "icon" to name) } }
        Sheet.Border -> BorderSheet(ed, close)
        Sheet.NewTable -> NewTableSheet(ed, close)
        Sheet.Data -> DataSheet(ed, onPick = { pickData.launch(DATA_MIME_TYPES) }, onImages = { pickImages.launch(visual) }, onDismiss = close)
        null -> Unit
    }
}

@Composable
private fun TopBar(ed: EditorState, onClose: () -> Unit, onPrint: () -> Unit, onExport: () -> Unit, onCheck: () -> Unit, onSaveCopy: () -> Unit, onJobs: () -> Unit) {
    val doc = ed.doc ?: return
    val app = ed.app
    var menu by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(Ic.ChevLeft, onClose, Palette.text)
        // The title is the document menu, as the chevron in M2 shows.
        Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
            Column(
                Modifier.clip(RoundedCornerShape(10.dp)).clickable { menu = true }.padding(horizontal = 10.dp, vertical = 4.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(doc.name, style = Type.nav, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    LIcon(Ic.ChevDown, Palette.muted, 14.dp)
                }
                Text(stringResource(if (ed.saved) R.string.saved else R.string.not_saved), style = Type.small)
            }
            DropdownMenu(menu, { menu = false }) {
                @Composable
                fun item(icon: Ic, text: Int, enabled: Boolean = true, action: () -> Unit) = DropdownMenuItem(
                    { Text(stringResource(text)) },
                    {
                        menu = false
                        action()
                    },
                    enabled = enabled,
                    leadingIcon = { LIcon(icon, if (enabled) Palette.text2 else Palette.border, 20.dp) },
                )
                item(Ic.Paste, R.string.paste, app.clip != null || app.systemClipboard() != null) { ed.pasteAny() }
                item(Ic.Group, R.string.select_all, doc.elements.isNotEmpty()) { ed.selectAll() }
                item(Ic.Check, R.string.check_label, action = onCheck)
                item(Ic.Copy, R.string.save_copy, action = onSaveCopy)
                item(Ic.Download, R.string.export, action = onExport)
                item(Ic.Printer, R.string.jobs_title, action = onJobs)
            }
        }
        IconButton(Ic.Undo, ed::undo, Palette.text2, enabled = doc.canUndo)
        IconButton(Ic.Redo, ed::redo, Palette.text2, enabled = doc.canRedo)
        IconButton(Ic.Printer, onPrint, Palette.primary)
    }
}

@Composable
private fun PaperBar(text: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Row(
        modifier.clip(RoundedCornerShape(50)).background(Palette.surface).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        LIcon(Ic.Rect, Palette.text, 16.dp)
        Text(text, style = Type.bodyStrong.copy(fontSize = 14.sp))
        LIcon(Ic.ChevDown, Palette.muted, 14.dp)
    }
}

@Composable
private fun Inspector(ed: EditorState, onBinding: () -> Unit, onMore: () -> Unit, onIcon: () -> Unit) {
    val e = ed.selected
    val several = ed.selectedElements
    Column(Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT).verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (e == null && several.isEmpty()) {
            Box(Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT - 32.dp), contentAlignment = Alignment.Center) {
                Text(stringResource(if (ed.multiSelect) R.string.inspector_hint_multi else R.string.inspector_hint), style = Type.caption, textAlign = TextAlign.Center)
            }
            return@Column
        }
        if (e == null) {
            MultiInspector(ed, several, onMore)
            return@Column
        }
        val bound = ed.boundColumn(e)
        // A binding can be chosen once there is data, and removed whenever there is one.
        val bindable = e.content != null && (ed.data != null || bound != null)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            IconTile(kindIcon(e.type))
            Column(Modifier.weight(1f)) {
                Text(elementTitle(e), style = Type.bodyStrong.copy(fontSize = 17.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOfNotNull(
                        kindName(e.type),
                        bound?.let { stringResource(R.string.bound_subtitle, it) },
                        stringResource(R.string.locked).takeIf { e.locked },
                        stringResource(R.string.degrees, mm(e.rotationDeg)).takeIf { e.rotationDeg != 0.0 },
                    ).joinToString(" · "),
                    style = Type.caption,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            IconButton(Ic.Copy, { ed.duplicateSelection() })
            IconButton(Ic.Trash, { ed.deleteSelection() }, Palette.red)
            IconButton(Ic.More, onMore, Palette.primary, fill = Palette.primaryTint)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LengthField("X", e.xMm, { ed.exec("element.move", "id" to e.id, "x_mm" to it, "y_mm" to e.yMm) }, Modifier.weight(1f), unit = false)
            LengthField("Y", e.yMm, { ed.exec("element.move", "id" to e.id, "x_mm" to e.xMm, "y_mm" to it) }, Modifier.weight(1f), unit = false)
            LengthField(stringResource(R.string.width_short), e.widthMm, { ed.exec("element.resize", "id" to e.id, "width_mm" to it, "height_mm" to e.heightMm) }, Modifier.weight(1f), unit = false)
            LengthField(stringResource(R.string.height_short), e.heightMm, { ed.exec("element.resize", "id" to e.id, "width_mm" to e.widthMm, "height_mm" to it) }, Modifier.weight(1f), unit = false)
        }
        when (e.type) {
            "text" -> TextProps(ed, e, bound, onBinding.takeIf { bindable })
            "barcode", "code2d" -> {
                // The value, symbology and text options are edited in the code sheet (M6).
                PickerRow(Ic.Barcode.takeIf { e.type == "barcode" } ?: Ic.Qr, e.data.orEmpty().ifEmpty { stringResource(R.string.data_hint) }, symbologyName(e)) { ed.codeSheet = e.id }
                if (bindable) BindingRow(bound, onBinding)
            }
            "image" -> ImageProps(ed, e)
            "table" -> PickerRow(Ic.Table, stringResource(R.string.table_edit), e.raw.optJSONArray("row_heights")?.length()?.let { r -> "$r × ${e.raw.optJSONArray("column_widths")?.length() ?: 0}" }) { ed.tableSheet = e.id }
            "icon" -> PickerRow(Ic.Star, e.raw.optString("icon").replace('_', ' '), null, onClick = onIcon)
            "rect", "ellipse", "line", "border" -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                LengthField(stringResource(R.string.prop_stroke), e.strokeMm, { ed.setProps(e.id, "stroke_mm" to it.coerceAtLeast(if (e.type == "line" || e.type == "border") 0.05 else 0.0)) }, Modifier.weight(1f))
                if (e.type == "rect" || e.type == "ellipse") Toggle(Ic.Contrast, e.fill) { ed.setProps(e.id, "fill" to !e.fill) }
                if (e.type == "border") PickerRow(Ic.Border, borderStyleName(e.raw.optString("style", "single")), null, Modifier.weight(1f), onClick = onMore)
            }
            else -> Unit
        }
    }
}

/** Several elements: how many, and aligning, distributing, grouping, locking and hiding them. */
@Composable
private fun MultiInspector(ed: EditorState, selected: List<Element>, onMore: () -> Unit) {
    val n = selected.size
    val ids = selected.map { it.id }
    val movable = selected.any { !it.locked }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        IconTile(Ic.Layers)
        Column(Modifier.weight(1f)) {
            Text(pluralStringResource(R.plurals.selected_count, n, n), style = Type.bodyStrong.copy(fontSize = 17.sp), maxLines = 1)
            boundsOf(selected)?.let { Text(sizeText(round1(it.width), round1(it.height)), style = Type.caption, maxLines = 1) }
        }
        IconButton(Ic.Copy, { ed.duplicateSelection() })
        IconButton(Ic.Trash, { ed.deleteSelection() }, Palette.red)
        IconButton(Ic.More, onMore, Palette.primary, fill = Palette.primaryTint)
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Toggle(Ic.AlignLeft, false, movable) { ed.align("left") }
        Toggle(Ic.AlignCenter, false, movable) { ed.align("center") }
        Toggle(Ic.AlignRight, false, movable) { ed.align("right") }
        Toggle(Ic.AlignTop, false, movable) { ed.align("top") }
        Toggle(Ic.AlignMiddle, false, movable) { ed.align("middle") }
        Toggle(Ic.AlignBottom, false, movable) { ed.align("bottom") }
    }
    val allLocked = selected.all { it.locked }
    val allHidden = selected.all { it.hidden }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Toggle(Ic.DistH, false, n >= 3) { ed.distribute("horizontal") }
        Toggle(Ic.DistV, false, n >= 3) { ed.distribute("vertical") }
        Toggle(Ic.Group, false, n >= 2) { ed.group() }
        Toggle(Ic.Ungroup, false, hasGroup(ids, selected)) { ed.ungroup() }
        Toggle(if (allLocked) Ic.Lock else Ic.Unlock, allLocked) { ed.lock(ids, !allLocked) }
        Toggle(if (allHidden) Ic.EyeOff else Ic.Eye, allHidden) { ed.setHidden(ids, !allHidden) }
    }
}

@Composable
private fun TextProps(ed: EditorState, e: Element, bound: String?, onBinding: (() -> Unit)?) {
    // Enter starts a new line; the text is committed when the keyboard closes.
    ContentField(e.text.orEmpty(), stringResource(R.string.text_hint), lines = 2) { ed.setText(e, it) }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        val size = e.fontSizePt
        Stepper(
            size.roundToInt().toString(),
            onMinus = { ed.setProps(e.id, "font_size_pt" to (size - 1).coerceAtLeast(4.0)) },
            onPlus = { ed.setProps(e.id, "font_size_pt" to (size + 1).coerceAtMost(200.0)) },
        )
        if (onBinding != null) BindingRow(bound, onBinding, Modifier.weight(1f), compact = true) else Box(Modifier.weight(1f))
        Toggle(Ic.Bold, e.bold) { ed.setProps(e.id, "bold" to !e.bold) }
        val (icon, next) = when (e.align) {
            "center" -> Ic.TCenter to "right"
            "right" -> Ic.TRight to "justify"
            "justify" -> Ic.Justify to "left"
            else -> Ic.TLeft to "center"
        }
        Toggle(icon, false) { ed.setProps(e.id, "align" to next) }
    }
}

/** The "Binding" row of the inspector: the bound column, or the word itself; opens the column picker. */
@Composable
private fun BindingRow(bound: String?, onClick: () -> Unit, modifier: Modifier = Modifier.fillMaxWidth(), compact: Boolean = false) {
    PickerRow(Ic.Link, bound ?: stringResource(R.string.binding), null, modifier, tint = if (bound != null) Palette.primary else Palette.text2, compact = compact, onClick = onClick)
}

/** What the engine has for the 1-bit conversion of an image: dither mode, threshold, invert. */
@Composable
private fun ImageProps(ed: EditorState, e: Element) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        var menu by remember { mutableStateOf(false) }
        Box(Modifier.weight(1f)) {
            PickerRow(Ic.Image, ditherName(e.dither), stringResource(R.string.image_dither)) { menu = true }
            DropdownMenu(menu, { menu = false }) {
                DITHERS.forEach { d ->
                    DropdownMenuItem(
                        { Text(ditherName(d)) },
                        {
                            menu = false
                            if (d != e.dither) ed.setProps(e.id, "dither" to d)
                        },
                    )
                }
            }
        }
        Toggle(Ic.Contrast, e.invert) { ed.setProps(e.id, "invert" to !e.invert) }
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        var threshold by remember(e.id, e.threshold) { mutableFloatStateOf(e.threshold.toFloat()) }
        Text(stringResource(R.string.image_threshold), style = Type.caption)
        Slider(
            value = threshold,
            onValueChange = { threshold = it },
            onValueChangeFinished = { if (threshold.roundToInt() != e.threshold) ed.setProps(e.id, "threshold" to threshold.roundToInt()) },
            valueRange = 0f..255f,
            colors = SliderDefaults.colors(thumbColor = Palette.primary, activeTrackColor = Palette.primary, inactiveTrackColor = Palette.primaryTint2),
            modifier = Modifier.weight(1f).height(44.dp),
        )
        Text(threshold.roundToInt().toString(), style = Type.bodyStrong, textAlign = TextAlign.End, modifier = Modifier.width(32.dp))
    }
}

@Composable
private fun Toolbar(data: Boolean, onInsert: () -> Unit, onLayers: () -> Unit, onData: () -> Unit, onTemplate: () -> Unit, onLabel: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(64.dp), verticalAlignment = Alignment.CenterVertically) {
        ToolbarItem(Ic.Plus, stringResource(R.string.tool_insert), onClick = onInsert)
        ToolbarItem(Ic.Layers, stringResource(R.string.tool_layers), onClick = onLayers)
        ToolbarItem(Ic.Data, stringResource(R.string.tool_data), selected = data, onClick = onData)
        ToolbarItem(Ic.Templates, stringResource(R.string.tool_template), onClick = onTemplate)
        ToolbarItem(Ic.Settings, stringResource(R.string.tool_label), onClick = onLabel)
    }
}

@Composable
private fun RowScope.ToolbarItem(icon: Ic, label: String, selected: Boolean = false, onClick: () -> Unit) {
    Column(
        Modifier.weight(1f).clickable(onClick = onClick).padding(vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        // The open panel's tab is tinted, as "Adatok" in M4.
        Box(
            Modifier.clip(RoundedCornerShape(10.dp)).background(if (selected) Palette.primaryTint else Color.Transparent).padding(horizontal = 14.dp, vertical = 2.dp),
        ) { LIcon(icon, if (selected) Palette.primary else Palette.text2, 24.dp) }
        Text(
            label,
            style = Type.small.copy(color = if (selected) Palette.primaryDark else Palette.text2, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium, fontSize = 12.sp),
            maxLines = 1,
        )
    }
}

private val INSPECTOR_HEIGHT = 256.dp
