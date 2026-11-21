package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.R
import org.labely.app.ui.LSwitch
import org.labely.app.ui.ListRow
import org.labely.app.ui.Segmented
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** The small controls the editor's inspector and sheets are built from, in the design's style. */

/** Square icon button that shows an on/off state, as Bold in M2. */
@Composable
internal fun Toggle(icon: Ic, on: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Palette.primaryTint else Palette.bg).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { LIcon(icon, if (!enabled) Palette.border else if (on) Palette.primary else Palette.text2, 22.dp) }
}

/** An action with its name under the icon, filling its share of a row. */
@Composable
internal fun RowScope.Action(icon: Ic, label: String, enabled: Boolean = true, on: Boolean = false, tint: Color = Palette.text2, onClick: () -> Unit) {
    Column(
        Modifier.weight(1f).clip(RoundedCornerShape(12.dp)).background(if (on) Palette.primaryTint else Palette.bg).clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        LIcon(icon, if (!enabled) Palette.border else if (on) Palette.primary else tint, 22.dp)
        Text(label, style = Type.small.copy(color = if (enabled) Palette.text2 else Palette.muted, fontSize = 11.sp), maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
    }
}

/** A row of [Action]s with the design's spacing. */
@Composable
internal fun Actions(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), content = content)
}

/** Title above a group of controls in a sheet. */
@Composable
internal fun Heading(text: String) {
    Text(text, style = Type.bodyStrong.copy(color = Palette.text2))
}

/**
 * A bordered row that opens a picker or a sheet: icon, value, optional detail and a chevron.
 * [compact] leaves the chevron out, for the narrow slot in the text row.
 */
@Composable
internal fun PickerRow(
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

/**
 * Content editor, committed on Done or when focus leaves ([live]: on every change); [trailing]
 * sits at its end. With [lines] above 1 it grows to that many lines and Enter starts a new line.
 */
@Composable
internal fun ContentField(value: String, placeholder: String, trailing: (@Composable () -> Unit)? = null, lines: Int = 1, live: Boolean = false, onCommit: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    val shape = RoundedCornerShape(12.dp)
    val multi = lines > 1
    Row(
        Modifier.fillMaxWidth().heightIn(min = 44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape)
            .padding(start = 12.dp, end = if (trailing != null) 2.dp else 12.dp, top = if (multi) 10.dp else 0.dp, bottom = if (multi) 10.dp else 0.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (text.isEmpty()) Text(placeholder, style = Type.body.copy(color = Palette.muted))
            BasicTextField(
                text,
                {
                    text = it
                    // A form field reports every keystroke; an element's property waits for Done.
                    if (live) onCommit(it)
                },
                singleLine = !multi,
                maxLines = if (multi) lines else 1,
                textStyle = Type.body,
                cursorBrush = SolidColor(Palette.text),
                keyboardOptions = KeyboardOptions(imeAction = if (multi) ImeAction.Default else ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (text != value) onCommit(text) }),
                modifier = Modifier.fillMaxWidth().onFocusChanged { if (!it.isFocused && text != value) onCommit(text) },
            )
        }
        trailing?.invoke()
    }
}

/** A list row with a switch. */
@Composable
internal fun SwitchRow(title: String, subtitle: String? = null, on: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    ListRow(title, subtitle, onClick = if (enabled) ({ onChange(!on) }) else null) { LSwitch(on, enabled, onChange) }
}

/** A list row with a segmented choice. */
@Composable
internal fun ChoiceRow(title: String, options: List<String>, selected: Int, onSelect: (Int) -> Unit) {
    ListRow(title) { Segmented(options, selected, scroll = options.size > 3, onSelect = onSelect) }
}

/** A list row that shows a value and opens a menu of [options] (value to label). */
@Composable
internal fun MenuRow(title: String, value: String, options: List<Pair<String, String>>, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        ListRow(title, onClick = { open = true }) {
            Text(value, style = Type.body.copy(color = Palette.muted), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.width(150.dp), textAlign = TextAlign.End)
            LIcon(Ic.ChevRight, Palette.muted, 18.dp)
        }
        DropdownMenu(open, { open = false }) {
            options.forEach { (id, label) ->
                DropdownMenuItem(
                    { Text(label) },
                    {
                        open = false
                        onSelect(id)
                    },
                )
            }
        }
    }
}

/** A list row with a slider from [min] to [max]; the value is committed when the thumb is let go. */
@Composable
internal fun SliderRow(title: String, value: Float, min: Float, max: Float, shown: (Float) -> String, onDone: (Float) -> Unit) {
    var v by remember(value) { mutableFloatStateOf(value) }
    ListRow(title) {
        Slider(
            value = v,
            onValueChange = { v = it },
            onValueChangeFinished = { if (v != value) onDone(v) },
            valueRange = min..max,
            colors = SliderDefaults.colors(thumbColor = Palette.primary, activeTrackColor = Palette.primary, inactiveTrackColor = Palette.primaryTint2),
            modifier = Modifier.width(140.dp),
        )
        Text(shown(v), style = Type.bodyStrong, textAlign = TextAlign.End, modifier = Modifier.width(44.dp))
    }
}

/** A dialog asking for one line of text. */
@Composable
fun TextDialog(title: String, label: String, initial: String, confirm: String, onDismiss: () -> Unit, onDone: (String) -> Unit) {
    var text by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { OutlinedTextField(text, { text = it }, singleLine = true, label = { Text(label) }) },
        confirmButton = { TextButton({ onDone(text.trim()) }, enabled = text.isNotBlank()) { Text(confirm) } },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** A yes/no question; [danger] colours the confirming button red. */
@Composable
fun ConfirmDialog(title: String, body: String, confirm: String, danger: Boolean = false, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(body) },
        confirmButton = { TextButton(onConfirm) { Text(confirm, color = if (danger) Palette.red else Palette.primary) } },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
