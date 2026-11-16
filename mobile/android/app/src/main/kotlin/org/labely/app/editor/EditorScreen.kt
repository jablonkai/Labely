package org.labely.app.editor

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
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

private enum class Sheet { Insert, Layers, Label, SaveTemplate, Print }

/** M2 · Szerkesztő: canvas, inspector of the selected element and the editing toolbar. */
@Composable
fun EditorScreen(ed: EditorState, onClose: () -> Unit) {
    var sheet by remember { mutableStateOf<Sheet?>(null) }
    BackHandler { if (ed.selectedId != null) ed.selectedId = null else onClose() }
    val doc = ed.doc ?: return

    Column(Modifier.fillMaxSize().background(Palette.surface).statusBarsPadding().navigationBarsPadding()) {
        TopBar(ed, onClose, onPrint = { sheet = Sheet.Print })
        Box(Modifier.weight(1f).fillMaxWidth().background(Palette.canvas)) {
            LabelCanvas(ed, Modifier.fillMaxSize().padding(bottom = 56.dp))
            PaperBar(sizeText(doc.label.widthMm, doc.label.heightMm) + " · " + mediaName(doc.label.media), Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp)) {
                sheet = Sheet.Label
            }
        }
        HorizontalDivider(color = Palette.border)
        Inspector(ed)
        HorizontalDivider(color = Palette.border)
        Toolbar(
            onInsert = { sheet = Sheet.Insert },
            onLayers = { sheet = Sheet.Layers },
            onTemplate = { sheet = Sheet.SaveTemplate },
            onLabel = { sheet = Sheet.Label },
        )
    }

    val close = { sheet = null }
    when (sheet) {
        Sheet.Insert -> InsertSheet(ed, close)
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
        null -> Unit
    }
}

@Composable
private fun TopBar(ed: EditorState, onClose: () -> Unit, onPrint: () -> Unit) {
    val doc = ed.doc ?: return
    Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(Ic.ChevLeft, onClose, Palette.text)
        Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(doc.name, style = Type.nav, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(stringResource(if (ed.saved) R.string.saved else R.string.not_saved), style = Type.small)
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
private fun Inspector(ed: EditorState) {
    val e = ed.selected
    Column(Modifier.fillMaxWidth().height(INSPECTOR_HEIGHT).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (e == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.inspector_hint), style = Type.caption, textAlign = TextAlign.Center)
            }
            return@Column
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            IconTile(kindIcon(e.type))
            Column(Modifier.weight(1f)) {
                Text(elementTitle(e), style = Type.bodyStrong.copy(fontSize = 17.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(kindName(e.type), style = Type.caption)
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
            "text" -> TextProps(ed, e)
            "barcode", "code2d" -> ContentField(e.data.orEmpty(), stringResource(R.string.data_hint)) { ed.setProps(e.id, "data" to it) }
            else -> Unit
        }
    }
}

@Composable
private fun TextProps(ed: EditorState, e: Element) {
    ContentField(e.text.orEmpty(), stringResource(R.string.text_hint)) { ed.setProps(e.id, "text" to it) }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        val size = e.fontSizePt
        Stepper(
            size.roundToInt().toString(),
            onMinus = { ed.setProps(e.id, "font_size_pt" to (size - 1).coerceAtLeast(4.0)) },
            onPlus = { ed.setProps(e.id, "font_size_pt" to (size + 1).coerceAtMost(200.0)) },
        )
        Box(Modifier.weight(1f))
        Toggle(Ic.Bold, e.bold) { ed.setProps(e.id, "bold" to !e.bold) }
        val (icon, next) = when (e.align) {
            "center" -> Ic.TCenter to "right"
            "right" -> Ic.TRight to "left"
            else -> Ic.TLeft to "center"
        }
        Toggle(icon, false) { ed.setProps(e.id, "align" to next) }
    }
}

@Composable
private fun Toggle(icon: Ic, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Palette.primaryTint else Palette.bg).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { LIcon(icon, if (on) Palette.primary else Palette.text2, 22.dp) }
}

/** Single-line content editor, committed on Done or when focus leaves. */
@Composable
private fun ContentField(value: String, placeholder: String, onCommit: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    val shape = RoundedCornerShape(12.dp)
    Box(
        Modifier.fillMaxWidth().height(44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape).padding(horizontal = 12.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
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
}

@Composable
private fun Toolbar(onInsert: () -> Unit, onLayers: () -> Unit, onTemplate: () -> Unit, onLabel: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(64.dp), verticalAlignment = Alignment.CenterVertically) {
        ToolbarItem(Ic.Plus, stringResource(R.string.tool_insert), onInsert)
        ToolbarItem(Ic.Layers, stringResource(R.string.tool_layers), onLayers)
        ToolbarItem(Ic.Templates, stringResource(R.string.tool_template), onTemplate)
        ToolbarItem(Ic.Settings, stringResource(R.string.tool_label), onLabel)
    }
}

@Composable
private fun RowScope.ToolbarItem(icon: Ic, label: String, onClick: () -> Unit) {
    Column(
        Modifier.weight(1f).clickable(onClick = onClick).padding(vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        LIcon(icon, Palette.text2, 24.dp)
        Text(label, style = Type.small.copy(color = Palette.text2, fontWeight = FontWeight.Medium, fontSize = 12.sp))
    }
}

private val INSPECTOR_HEIGHT = 256.dp
