package org.labely.app.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale
import org.labely.app.editor.Units
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** The unit lengths are shown and typed in; the app's settings set it. */
object Lengths {
    var units by mutableStateOf(Units.Mm)
}

/** `50 × 30 mm` or `1.969 × 1.181 in`, in the chosen unit. */
fun sizeText(w: Double, h: Double): String = Lengths.units.let { "${it.format(w)} × ${it.format(h)} ${it.id}" }

/** A length with its unit: `12.5 mm`. */
fun lengthText(mm: Double): String = Lengths.units.let { "${it.format(mm)} ${it.id}" }

fun mm(v: Double): String =
    if (v == Math.rint(v)) v.toLong().toString() else String.format(Locale.getDefault(), "%.1f", v)

/** White rounded card with a hairline border, as in the design. */
@Composable
fun Card(
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier
            .clip(shape)
            .background(Palette.surface)
            .border(if (selected) 2.dp else 1.dp, if (selected) Palette.primary else Palette.border, shape)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier),
        content = content,
    )
}

@Composable
fun SectionHeader(title: String, action: String? = null, onAction: () -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.title.copy(fontSize = 20.sp), modifier = Modifier.weight(1f))
        if (action != null) {
            Text(action, style = Type.body.copy(color = Palette.primary, fontWeight = FontWeight.SemiBold), modifier = Modifier.clickable(onClick = onAction))
        }
    }
}

@Composable
fun LargeTitle(title: String, sub: String? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.Bottom) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = Type.largeTitle)
            if (sub != null) Text(sub, style = Type.caption)
        }
        trailing()
    }
}

@Composable
fun SearchField(value: String, placeholder: String, onChange: (String) -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(12.dp)).background(Palette.field).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        LIcon(Ic.Search, Palette.muted, 18.dp)
        Box(Modifier.weight(1f)) {
            if (value.isEmpty()) Text(placeholder, style = Type.body.copy(color = Palette.muted), maxLines = 1)
            BasicTextField(value, onChange, singleLine = true, textStyle = Type.body, cursorBrush = SolidColor(Palette.text), modifier = Modifier.fillMaxWidth())
        }
        if (value.isNotEmpty()) LIcon(Ic.Close, Palette.muted, 16.dp, Modifier.clickable { onChange("") })
    }
}

@Composable
fun Chip(text: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(50)
    Text(
        text,
        style = Type.body.copy(color = if (selected) Palette.primaryDark else Palette.text, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium),
        modifier = Modifier
            .clip(shape)
            .background(if (selected) Palette.primaryTint else Palette.surface)
            .border(1.dp, if (selected) Palette.primaryTint2 else Palette.border, shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 8.dp),
    )
}

/** Round icon button; [tint] colours the glyph, [fill] the circle. */
@Composable
fun IconButton(icon: Ic, onClick: () -> Unit, tint: Color = Palette.text2, fill: Color = Color.Transparent, size: Dp = 40.dp, enabled: Boolean = true) {
    Box(
        Modifier.size(size).clip(CircleShape).background(fill).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { LIcon(icon, if (enabled) tint else Palette.border, size * 0.55f) }
}

@Composable
fun PrimaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, icon: Ic? = null, enabled: Boolean = true) {
    Row(
        modifier.height(52.dp).clip(RoundedCornerShape(14.dp)).background(if (enabled) Palette.primary else Palette.primaryTint2)
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) LIcon(icon, Color.White, 20.dp, Modifier.padding(end = 8.dp))
        Text(text, style = Type.bodyStrong.copy(color = Color.White, fontSize = 17.sp))
    }
}

@Composable
fun SecondaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, icon: Ic? = null, enabled: Boolean = true) {
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier.height(44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape)
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 12.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) LIcon(icon, Palette.text, 18.dp, Modifier.padding(end = 6.dp))
        Text(text, style = Type.bodyStrong.copy(color = if (enabled) Palette.text else Palette.muted))
    }
}

/** A row of a grouped list: title, optional subtitle, trailing content. */
@Composable
fun ListRow(
    title: String,
    subtitle: String? = null,
    onClick: (() -> Unit)? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        leading?.invoke()
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) Text(subtitle, style = Type.caption, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        trailing()
    }
}

@Composable
fun RowDivider() = HorizontalDivider(color = Palette.borderSoft)

/** Icon in a tinted rounded square, as in list rows and cards. */
@Composable
fun IconTile(icon: Ic, tint: Color = Palette.primary, fill: Color = Palette.primaryTint, size: Dp = 40.dp) {
    Box(Modifier.size(size).clip(RoundedCornerShape(10.dp)).background(fill), contentAlignment = Alignment.Center) {
        LIcon(icon, tint, size * 0.5f)
    }
}

/** − value + stepper. */
@Composable
fun Stepper(value: String, onMinus: () -> Unit, onPlus: () -> Unit) {
    Row(
        Modifier.clip(RoundedCornerShape(12.dp)).background(Palette.bg).padding(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(Ic.Minus, onMinus, Palette.text2, size = 36.dp)
        Text(value, style = Type.bodyStrong.copy(fontSize = 17.sp), modifier = Modifier.widthIn(min = 36.dp).padding(horizontal = 4.dp), maxLines = 1)
        IconButton(Ic.Plus, onPlus, Palette.text2, size = 36.dp)
    }
}

/**
 * Bordered numeric field with a leading label and a `mm` suffix. The value is committed when
 * the keyboard's Done is pressed or the field loses focus.
 */
@Composable
fun NumberField(label: String, value: Double, onCommit: (Double) -> Unit, modifier: Modifier = Modifier, suffix: String = "mm", shown: String = mm(value)) {
    var text by remember(value, shown) { mutableStateOf(shown) }
    fun commit() {
        // Only what the user typed is committed, so a rounded display never changes the value.
        if (text != shown) text.replace(',', '.').toDoubleOrNull()?.let { if (it != value) onCommit(it) }
    }
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier.height(44.dp).clip(shape).background(Palette.surface).border(1.dp, Palette.border, shape).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(label, style = Type.caption)
        BasicTextField(
            text,
            { text = it },
            singleLine = true,
            textStyle = Type.body,
            cursorBrush = SolidColor(Palette.text),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { commit() }),
            modifier = Modifier.weight(1f).onFocusChanged { if (!it.isFocused) commit() },
        )
        Text(suffix, style = Type.caption)
    }
}

/** [NumberField] for a length: shown and typed in the chosen unit, committed in millimetres. */
@Composable
fun LengthField(label: String, valueMm: Double, onCommit: (Double) -> Unit, modifier: Modifier = Modifier, unit: Boolean = true) {
    val u = Lengths.units
    NumberField(label, u.fromMm(valueMm), { onCommit(u.toMm(it)) }, modifier, if (unit) u.id else "", u.format(valueMm))
}

/** A label raster on a white card, sized to its aspect ratio. */
@Composable
fun LabelImage(image: ImageBitmap?, widthMm: Double, heightMm: Double, modifier: Modifier = Modifier) {
    val ratio = (widthMm / heightMm).toFloat().coerceIn(0.2f, 5f)
    Surface(modifier.aspectRatio(ratio), shape = RoundedCornerShape(8.dp), color = Color.White, shadowElevation = 2.dp) {
        if (image != null) {
            Image(image, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit, filterQuality = FilterQuality.Medium)
        }
    }
}

/** Grey tile holding a label thumbnail, as in the template cards. */
@Composable
fun ThumbTile(image: ImageBitmap?, widthMm: Double, heightMm: Double) {
    Box(
        Modifier.fillMaxWidth().aspectRatio(1.7f).clip(RoundedCornerShape(12.dp)).background(Palette.canvas).padding(12.dp),
        contentAlignment = Alignment.Center,
    ) { LabelImage(image, widthMm, heightMm, Modifier.shadow(0.dp)) }
}

/** Header of a bottom sheet: title and a round close button. */
@Composable
fun SheetHeader(title: String, onClose: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.title, modifier = Modifier.weight(1f))
        IconButton(Ic.Close, onClose, Palette.text2, fill = Palette.bg, size = 36.dp)
    }
}

/** Green or amber status line with an icon, e.g. the check result. */
@Composable
fun Banner(text: String, ok: Boolean, trailing: String? = null) {
    val fg = if (ok) Palette.green else Palette.amber
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(if (ok) Palette.greenTint else Palette.amberTint).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        LIcon(if (ok) Ic.Check else Ic.Warn, fg, 18.dp)
        Text(text, style = Type.bodyStrong.copy(color = fg), modifier = Modifier.weight(1f))
        if (trailing != null) Text(trailing, style = Type.bodyStrong)
    }
}

/** Grouped list container. */
@Composable
fun Group(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    Column(modifier.fillMaxWidth().clip(shape).background(Palette.surface).border(BorderStroke(1.dp, Palette.border), shape), content = content)
}

/** Segmented control. With [scroll] it scrolls sideways when the options do not fit. */
@Composable
fun Segmented(options: List<String>, selected: Int, modifier: Modifier = Modifier, scroll: Boolean = false, onSelect: (Int) -> Unit) {
    Row(
        modifier.clip(RoundedCornerShape(10.dp)).background(Palette.bg)
            .then(if (scroll) Modifier.horizontalScroll(rememberScrollState()) else Modifier).padding(3.dp),
    ) {
        options.forEachIndexed { i, o ->
            val on = i == selected
            Text(
                o,
                style = Type.bodyStrong.copy(fontSize = 14.sp, color = if (on) Palette.text else Palette.muted, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Medium),
                maxLines = 1,
                modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (on) Palette.surface else Palette.bg)
                    .clickable { onSelect(i) }.padding(horizontal = 14.dp, vertical = 7.dp),
            )
        }
    }
}

/** On/off switch in the design's colours. */
@Composable
fun LSwitch(on: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    Switch(
        on,
        onChange,
        enabled = enabled,
        colors = SwitchDefaults.colors(
            checkedThumbColor = Color.White,
            checkedTrackColor = Palette.primary,
            uncheckedThumbColor = Color.White,
            uncheckedTrackColor = Palette.border,
            uncheckedBorderColor = Palette.border,
        ),
    )
}
