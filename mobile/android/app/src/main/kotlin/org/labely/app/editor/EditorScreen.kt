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
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.NumberField
import org.labely.app.ui.Stepper
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

private enum class Sheet { Insert, Layers, Label, SaveTemplate, Print, Export, Binding }

/** M2 · Szerkesztő: canvas, inspector of the selected element and the editing toolbar. */
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
    LaunchedEffect(ed.selectedId) { if (ed.selectedId != null) dataPanel = false }
    BackHandler {
        when {
            dataPanel -> dataPanel = false
            ed.selectedId != null -> ed.selectedId = null
            else -> onClose()
        }
    }
    val doc = ed.doc ?: return

    val context = LocalContext.current.applicationContext
    val app = ed.app
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

    Column(Modifier.fillMaxSize().background(Palette.surface).statusBarsPadding().navigationBarsPadding()) {
        TopBar(ed, onClose, onPrint = { open(Sheet.Print) }, onExport = { open(Sheet.Export) })
        Box(Modifier.weight(1f).fillMaxWidth().background(Palette.canvas)) {
            LabelCanvas(ed, Modifier.fillMaxSize().padding(bottom = 56.dp))
            PaperBar(sizeText(doc.label.widthMm, doc.label.heightMm) + " · " + mediaName(doc.label.media), Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp)) {
                open(Sheet.Label)
            }
        }
        HorizontalDivider(color = Palette.border)
        if (dataPanel) {
            DataPanel(ed, Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT)) { pickData.launch(DATA_MIME_TYPES) }
        } else {
            Inspector(ed, onBinding = { open(Sheet.Binding) })
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
    // The code sheet is under the binding picker it can open.
    ed.codeSheet?.let { id -> CodeSheet(ed, id, onBinding = { sheet = Sheet.Binding }) { ed.codeSheet = null } }
    when (sheet) {
        Sheet.Insert -> InsertSheet(ed, onImage = { pickImage.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }, close)
        Sheet.Layers -> LayersSheet(ed, close)
        Sheet.Label -> LabelSheet(
            title = stringResource(R.string.label_title),
            confirm = stringResource(R.string.apply),
            initial = doc.label.let { Triple(it.widthMm, it.heightMm, it.media) },
            onDismiss = close,
        ) { w, h, media ->
            ed.setLabel(w, h, media)
            close()
        }
        Sheet.SaveTemplate -> SaveTemplateDialog(doc.name, close) {
            ed.saveTemplate(it)
            close()
        }
        Sheet.Print -> PrintSheet(ed, close)
        Sheet.Export -> ExportSheet(ed, close)
        Sheet.Binding -> {
            val e = ed.doc?.elements?.firstOrNull { it.id == (ed.codeSheet ?: ed.selectedId) }
            if (e != null) BindingSheet(ed, e, close) else LaunchedEffect(Unit) { close() }
        }
        null -> Unit
    }
}

@Composable
private fun TopBar(ed: EditorState, onClose: () -> Unit, onPrint: () -> Unit, onExport: () -> Unit) {
    val doc = ed.doc ?: return
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
                DropdownMenuItem(
                    { Text(stringResource(R.string.export)) },
                    {
                        menu = false
                        onExport()
                    },
                    leadingIcon = { LIcon(Ic.Download, Palette.text2, 20.dp) },
                )
            }
        }
        IconButton(Ic.Undo, ed::undo, Palette.text2, enabled = doc.canUndo)
        IconButton(Ic.Redo, ed::redo, Palette.text2, enabled = doc.canRedo)
        IconButton(Ic.Printer, onPrint, Palette.primary)
    }
}

@Composable
private fun PaperBar(text: String, modifier: Modifier, onClick: () -> Unit) {
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
private fun Inspector(ed: EditorState, onBinding: () -> Unit) {
    val e = ed.selected
    Column(Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (e == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.inspector_hint), style = Type.caption, textAlign = TextAlign.Center)
            }
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
                    if (bound != null) kindName(e.type) + " · " + stringResource(R.string.bound_subtitle, bound) else kindName(e.type),
                    style = Type.caption,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            IconButton(Ic.Copy, { ed.exec("element.duplicate", "id" to e.id, "dx_mm" to 1.0, "dy_mm" to 1.0, select = true) })
            IconButton(Ic.Trash, { ed.exec("element.delete", "ids" to JSONArray(listOf(e.id))) }, Palette.red)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            NumberField("X", e.xMm, { ed.exec("element.move", "id" to e.id, "x_mm" to it, "y_mm" to e.yMm) }, Modifier.weight(1f), "")
            NumberField("Y", e.yMm, { ed.exec("element.move", "id" to e.id, "x_mm" to e.xMm, "y_mm" to it) }, Modifier.weight(1f), "")
            NumberField(stringResource(R.string.width_short), e.widthMm, { ed.exec("element.resize", "id" to e.id, "width_mm" to it, "height_mm" to e.heightMm) }, Modifier.weight(1f), "")
            NumberField(stringResource(R.string.height_short), e.heightMm, { ed.exec("element.resize", "id" to e.id, "width_mm" to e.widthMm, "height_mm" to it) }, Modifier.weight(1f), "")
        }
        when (e.type) {
            "text" -> TextProps(ed, e, bound, onBinding.takeIf { bindable })
            "barcode", "code2d" -> {
                // The value, symbology and text options are edited in the code sheet (M6).
                PickerRow(Ic.Barcode.takeIf { e.type == "barcode" } ?: Ic.Qr, e.data.orEmpty().ifEmpty { stringResource(R.string.data_hint) }, symbologyName(e)) { ed.codeSheet = e.id }
                if (bindable) BindingRow(bound, onBinding)
            }
            "image" -> ImageProps(ed, e)
            else -> Unit
        }
    }
}

@Composable
private fun TextProps(ed: EditorState, e: Element, bound: String?, onBinding: (() -> Unit)?) {
    ContentField(e.text.orEmpty(), stringResource(R.string.text_hint)) { ed.setProps(e.id, "text" to it) }
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
            "right" -> Ic.TRight to "left"
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

/**
 * A bordered row that opens a picker or a sheet: icon, value, optional detail and a chevron.
 * [compact] leaves the chevron out, for the narrow slot in the text row.
 */
@Composable
private fun PickerRow(
    icon: Ic,
    text: String,
    detail: String?,
    modifier: Modifier = Modifier.fillMaxWidth(),
    tint: Color = Palette.text2,
    compact: Boolean = false,
    onClick: () -> Unit,
) {
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier.height(44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape).clickable(onClick = onClick)
            .padding(horizontal = if (compact) 8.dp else 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(if (compact) 6.dp else 8.dp),
    ) {
        LIcon(icon, tint, 18.dp)
        Text(text, style = Type.body.copy(color = if (tint == Palette.primary) Palette.primaryDark else Palette.text), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (detail != null) Text(detail, style = Type.caption, maxLines = 1)
        if (!compact) LIcon(Ic.ChevRight, Palette.muted, 16.dp)
    }
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
private fun Toggle(icon: Ic, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Palette.primaryTint else Palette.bg).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { LIcon(icon, if (on) Palette.primary else Palette.text2, 22.dp) }
}

/** Single-line content editor, committed on Done or when focus leaves; [trailing] sits at its end. */
@Composable
internal fun ContentField(value: String, placeholder: String, trailing: (@Composable () -> Unit)? = null, onCommit: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    val shape = RoundedCornerShape(12.dp)
    Row(
        Modifier.fillMaxWidth().height(44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape).padding(start = 12.dp, end = if (trailing != null) 2.dp else 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (text.isEmpty()) Text(placeholder, style = Type.body.copy(color = Palette.muted))
            BasicTextField(
                text,
                { text = it },
                singleLine = true,
                textStyle = Type.body,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (text != value) onCommit(text) }),
                modifier = Modifier.fillMaxWidth().onFocusChanged { if (!it.isFocused && text != value) onCommit(text) },
            )
        }
        trailing?.invoke()
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
