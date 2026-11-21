package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
import org.json.JSONArray
import org.json.JSONObject
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.ui.Group
import org.labely.app.ui.LengthField
import org.labely.app.ui.ListRow
import org.labely.app.ui.NumberField
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.Stepper
import org.labely.app.ui.mm
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

val BORDER_STYLES = listOf("single", "double", "thick_thin", "dashed", "dotted", "corners", "ticket", "scallop")

@Composable
fun borderStyleName(style: String): String = stringResource(
    when (style) {
        "double" -> R.string.border_double
        "thick_thin" -> R.string.border_thick_thin
        "dashed" -> R.string.border_dashed
        "dotted" -> R.string.border_dotted
        "corners" -> R.string.border_corners
        "ticket" -> R.string.border_ticket
        "scallop" -> R.string.border_scallop
        else -> R.string.border_single
    },
)

/** Every property of element [e], by kind. Each change is one `element.set_props`. */
@Composable
internal fun PropertiesContent(ed: EditorState, e: Element, onBinding: () -> Unit, onPickIcon: () -> Unit, onAddFont: () -> Unit) {
    when (e.type) {
        "text" -> TextProperties(ed, e, onBinding, onAddFont)
        "barcode", "code2d" -> {
            PickerRow(if (e.type == "barcode") Ic.Barcode else Ic.Qr, e.data.orEmpty().ifEmpty { stringResource(R.string.data_hint) }, symbologyName(e)) { ed.codeSheet = e.id }
            CodeExtras(ed, e)
        }
        "image" -> ImageProperties(ed, e)
        "rect", "ellipse" -> Group {
            Field(stringResource(R.string.prop_stroke)) { LengthField("", e.strokeMm, { ed.setProps(e.id, "stroke_mm" to it.coerceAtLeast(0.0)) }, Modifier.weight(1f)) }
            RowDivider()
            SwitchRow(stringResource(R.string.prop_fill), on = e.fill) { ed.setProps(e.id, "fill" to it) }
            if (e.type == "rect") {
                RowDivider()
                Field(stringResource(R.string.prop_corner_radius)) { LengthField("", e.cornerRadiusMm, { ed.setProps(e.id, "corner_radius_mm" to it.coerceAtLeast(0.0)) }, Modifier.weight(1f)) }
            }
        }
        "line" -> {
            val dash = e.raw.optJSONArray("dash_mm")
            val style = LineStyle.of(dash?.optDouble(0), dash?.optDouble(1))
            fun set(stroke: Double, s: LineStyle) {
                val d = s.dash(stroke)
                ed.setRaw(e.id, JSONObject().put("stroke_mm", stroke).put("dash_mm", if (d == null) JSONObject.NULL else JSONArray(listOf(d.first, d.second))))
            }
            Group {
                Field(stringResource(R.string.prop_stroke)) { LengthField("", e.strokeMm, { set(it.coerceAtLeast(0.05), style) }, Modifier.weight(1f)) }
                RowDivider()
                ChoiceRow(
                    stringResource(R.string.prop_line_style),
                    listOf(stringResource(R.string.line_solid), stringResource(R.string.line_dashed), stringResource(R.string.line_dotted)),
                    style.ordinal,
                ) { set(e.strokeMm, LineStyle.entries[it]) }
            }
        }
        "icon" -> Group {
            ListRow(stringResource(R.string.kind_icon), e.raw.optString("icon").replace('_', ' '), onClick = onPickIcon) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
            RowDivider()
            SwitchRow(stringResource(R.string.prop_keep_aspect), on = e.raw.optBoolean("keep_aspect", true)) { ed.setProps(e.id, "keep_aspect" to it) }
            RowDivider()
            SwitchRow(stringResource(R.string.prop_invert), stringResource(R.string.prop_invert_sub), e.invert) { ed.setProps(e.id, "invert" to it) }
        }
        "border" -> Group {
            val style = e.raw.optString("style", "single")
            MenuRow(stringResource(R.string.prop_border_style), borderStyleName(style), BORDER_STYLES.map { it to borderStyleName(it) }) { if (it != style) ed.setProps(e.id, "style" to it) }
            RowDivider()
            Field(stringResource(R.string.prop_stroke)) { LengthField("", e.strokeMm, { ed.setProps(e.id, "stroke_mm" to it.coerceAtLeast(0.05)) }, Modifier.weight(1f)) }
            RowDivider()
            Field(stringResource(R.string.prop_corner_radius)) { LengthField("", e.cornerRadiusMm, { ed.setProps(e.id, "corner_radius_mm" to it.coerceAtLeast(0.0)) }, Modifier.weight(1f)) }
        }
        "table" -> {
            SecondaryButton(stringResource(R.string.table_edit), { ed.tableSheet = e.id }, Modifier.fillMaxWidth(), Ic.Table)
            Group {
                Field(stringResource(R.string.prop_grid_lines)) { LengthField("", e.strokeMm, { ed.setProps(e.id, "stroke_mm" to it.coerceAtLeast(0.0)) }, Modifier.weight(1f)) }
                RowDivider()
                FontRow(ed, e, onAddFont)
                RowDivider()
                ListRow(stringResource(R.string.prop_font_size)) { SizeStepper(e.fontSizePt) { ed.setProps(e.id, "font_size_pt" to it) } }
                RowDivider()
                ChoiceRow(stringResource(R.string.prop_align), listOf(stringResource(R.string.align_left), stringResource(R.string.align_center), stringResource(R.string.align_right)), H_ALIGN.indexOf(e.raw.optString("align", "center")).coerceAtLeast(0)) {
                    ed.setProps(e.id, "align" to H_ALIGN[it])
                }
            }
        }
        else -> Text(stringResource(R.string.prop_none), style = Type.caption)
    }
}

private val H_ALIGN = listOf("left", "center", "right")
private val V_ALIGN = listOf("top", "middle", "bottom")
private val FITS = listOf("none", "shrink", "fill")
val IMAGE_FITS = listOf("contain", "cover", "stretch")
val ECC_LEVELS = listOf("L", "M", "Q", "H")

/** A list row whose trailing part is an input field. */
@Composable
private fun Field(title: String, field: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(title, style = Type.bodyStrong.copy(fontSize = 16.sp), modifier = Modifier.weight(1f), maxLines = 1)
        field()
    }
}

@Composable
private fun SizeStepper(size: Double, onChange: (Double) -> Unit) {
    Stepper(mm(size), { onChange((size - 1).coerceAtLeast(4.0)) }, { onChange((size + 1).coerceAtMost(200.0)) })
}

/** The font family of a text or table, with "Add a font file…" at the end of the list. */
@Composable
private fun FontRow(ed: EditorState, e: Element, onAddFont: () -> Unit) {
    val add = stringResource(R.string.font_add)
    val fonts = (ed.app.fonts + e.fontFamily).distinct().sorted()
    MenuRow(stringResource(R.string.prop_font), e.fontFamily, fonts.map { it to it } + (ADD_FONT to add)) {
        if (it == ADD_FONT) onAddFont() else if (it != e.fontFamily) ed.setProps(e.id, "font_family" to it)
    }
}

private const val ADD_FONT = "\u0000add"

@Composable
private fun TextProperties(ed: EditorState, e: Element, onBinding: () -> Unit, onAddFont: () -> Unit) {
    val text = e.text.orEmpty()
    ContentField(text, stringResource(R.string.text_hint), lines = 5) { ed.setText(e, it) }
    // Computed values, as the studio's insert chips: appended to the text.
    Actions {
        Action(Ic.Calendar, stringResource(R.string.insert_date)) { ed.appendBinding(e, EditorState.DATE_BINDING) }
        Action(Ic.Clock, stringResource(R.string.insert_time)) { ed.appendBinding(e, EditorState.TIME_BINDING) }
        Action(Ic.Hash, stringResource(R.string.insert_serial)) { ed.appendBinding(e, EditorState.SERIAL_BINDING) }
        if (ed.data != null || ed.boundColumn(e) != null) Action(Ic.Link, stringResource(R.string.binding), on = ed.boundColumn(e) != null, onClick = onBinding)
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        SizeStepper(e.fontSizePt) { ed.setProps(e.id, "font_size_pt" to it) }
        Box(Modifier.weight(1f))
        Toggle(Ic.Bold, e.bold) { ed.setProps(e.id, "bold" to !e.bold) }
        Toggle(Ic.Italic, e.italic) { ed.setProps(e.id, "italic" to !e.italic) }
        Toggle(Ic.Underline, e.underline) { ed.setProps(e.id, "underline" to !e.underline) }
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Toggle(Ic.TLeft, e.align == "left") { ed.setProps(e.id, "align" to "left") }
        Toggle(Ic.TCenter, e.align == "center") { ed.setProps(e.id, "align" to "center") }
        Toggle(Ic.TRight, e.align == "right") { ed.setProps(e.id, "align" to "right") }
        Toggle(Ic.Justify, e.align == "justify") { ed.setProps(e.id, "align" to "justify") }
        Box(Modifier.weight(1f))
        Toggle(Ic.AlignTop, e.valign == "top") { ed.setProps(e.id, "valign" to "top") }
        Toggle(Ic.AlignMiddle, e.valign == "middle") { ed.setProps(e.id, "valign" to "middle") }
        Toggle(Ic.AlignBottom, e.valign == "bottom") { ed.setProps(e.id, "valign" to "bottom") }
    }
    Group {
        FontRow(ed, e, onAddFont)
        RowDivider()
        ChoiceRow(
            stringResource(R.string.prop_fit),
            listOf(stringResource(R.string.fit_fixed), stringResource(R.string.fit_shrink), stringResource(R.string.fit_fill)),
            FITS.indexOf(e.fit).coerceAtLeast(0),
        ) { ed.setProps(e.id, "fit" to FITS[it]) }
        RowDivider()
        SwitchRow(stringResource(R.string.prop_wrap), on = e.wrap) { ed.setProps(e.id, "wrap" to it) }
        RowDivider()
        SwitchRow(stringResource(R.string.prop_invert), stringResource(R.string.prop_invert_sub), e.invert) { ed.setProps(e.id, "invert" to it) }
        RowDivider()
        SwitchRow(stringResource(R.string.prop_vertical), stringResource(R.string.prop_vertical_sub), e.direction == "vertical") { ed.setProps(e.id, "direction" to if (it) "vertical" else "horizontal") }
        RowDivider()
        Field(stringResource(R.string.prop_line_spacing)) { NumberField("", e.lineSpacing, { ed.setProps(e.id, "line_spacing" to it.coerceIn(0.5, 5.0)) }, Modifier.weight(1f), "×") }
        RowDivider()
        Field(stringResource(R.string.prop_letter_spacing)) { LengthField("", e.letterSpacingMm, { ed.setProps(e.id, "letter_spacing_mm" to it) }, Modifier.weight(1f)) }
        RowDivider()
        // 0 switches the length check off.
        Field(stringResource(R.string.prop_max_length)) {
            LengthField("", e.maxLengthMm ?: 0.0, { ed.setRaw(e.id, JSONObject().put("max_length_mm", if (it > 0) it else JSONObject.NULL)) }, Modifier.weight(1f))
        }
    }
    if (e.direction != "vertical" && text.isNotEmpty()) RichText(ed, e)
}

/**
 * Style part of a text: select a range in the field, then make it bold, italic, underlined,
 * smaller or larger, or clear its styles. The ranges are the element's `spans`.
 */
@Composable
private fun RichText(ed: EditorState, e: Element) {
    val text = e.text.orEmpty()
    val spans = spansOf(e)
    var selection by remember(text) { mutableStateOf(TextRange.Zero) }
    val start = scalarIndex(text, selection.min)
    val end = scalarIndex(text, selection.max)
    val some = end > start
    // The style in force at the start of the selection decides what a toggle does.
    fun at(pick: (Span) -> Any?): Any? = spans.lastOrNull { it.start <= start && start < it.end && pick(it) != null }?.let(pick)
    val bold = at { it.bold } as Boolean? ?: e.bold
    val italic = at { it.italic } as Boolean? ?: e.italic
    val underline = at { it.underline } as Boolean? ?: e.underline
    val size = at { it.fontSizePt } as Double? ?: e.fontSizePt
    fun apply(change: (Span) -> Span) = ed.setSpans(e.id, styleRange(spans, start, end, change))

    Heading(stringResource(R.string.rich_title))
    val shape = RoundedCornerShape(12.dp)
    BasicTextField(
        value = TextFieldValue(text, selection),
        onValueChange = { selection = it.selection },
        readOnly = true,
        textStyle = Type.body,
        visualTransformation = { TransformedText(styled(text, spans, e), OffsetMapping.Identity) },
        modifier = Modifier.fillMaxWidth().clip(shape).background(Palette.bg).border(1.dp, Palette.border, shape).padding(12.dp),
    )
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Toggle(Ic.Bold, some && bold, some) { apply { it.copy(bold = !bold) } }
        Toggle(Ic.Italic, some && italic, some) { apply { it.copy(italic = !italic) } }
        Toggle(Ic.Underline, some && underline, some) { apply { it.copy(underline = !underline) } }
        Toggle(Ic.Minus, false, some) { apply { it.copy(fontSizePt = (size - 1).coerceAtLeast(4.0)) } }
        Toggle(Ic.Plus, false, some) { apply { it.copy(fontSizePt = (size + 1).coerceAtMost(200.0)) } }
        Box(Modifier.weight(1f))
        Text(
            stringResource(R.string.rich_clear),
            style = Type.bodyStrong.copy(color = if (spans.isEmpty()) Palette.muted else Palette.primary),
            modifier = Modifier.clickable(enabled = spans.isNotEmpty()) { if (some) apply { s -> Span(s.start, s.end) } else ed.setSpans(e.id, emptyList()) }.padding(8.dp),
        )
    }
    Text(stringResource(if (some) R.string.rich_hint_selected else R.string.rich_hint, mm(size)), style = Type.caption)
}

/** [text] with its style ranges shown: bold, italic, underline and relative size. */
private fun styled(text: String, spans: List<Span>, e: Element): AnnotatedString = buildAnnotatedString {
    append(text)
    for (s in spans) {
        val a = text.offsetByCodePoints(0, s.start.coerceIn(0, text.codePointCount(0, text.length)))
        val b = text.offsetByCodePoints(0, s.end.coerceIn(0, text.codePointCount(0, text.length)))
        if (b <= a) continue
        addStyle(
            SpanStyle(
                fontWeight = s.bold?.let { if (it) FontWeight.Bold else FontWeight.Normal },
                fontStyle = s.italic?.let { if (it) FontStyle.Italic else FontStyle.Normal },
                textDecoration = s.underline?.let { if (it) TextDecoration.Underline else TextDecoration.None },
                fontSize = s.fontSizePt?.let { (15 * it / e.fontSizePt).coerceIn(9.0, 30.0).sp } ?: androidx.compose.ui.unit.TextUnit.Unspecified,
            ),
            a,
            b,
        )
    }
}

/** A barcode's readable-text size, a QR code's error correction. */
@Composable
internal fun CodeExtras(ed: EditorState, e: Element) {
    if (e.type == "barcode" && e.humanReadable != "none") {
        Group {
            ListRow(stringResource(R.string.code_text_size)) {
                val size = e.raw.optDouble("text_size_pt", 8.0)
                Stepper(mm(size), { ed.setProps(e.id, "text_size_pt" to (size - 1).coerceAtLeast(4.0)) }, { ed.setProps(e.id, "text_size_pt" to (size + 1).coerceAtMost(72.0)) })
            }
        }
    }
    if (e.type == "code2d" && e.symbology == "qr") {
        Group {
            val ecc = e.raw.optString("ecc", "M")
            ListRow(stringResource(R.string.code_ecc), stringResource(R.string.code_ecc_sub))
            org.labely.app.ui.Segmented(ECC_LEVELS, ECC_LEVELS.indexOf(ecc).coerceAtLeast(0), Modifier.padding(start = 16.dp, end = 16.dp, bottom = 14.dp)) {
                if (ECC_LEVELS[it] != ecc) ed.setProps(e.id, "ecc" to ECC_LEVELS[it])
            }
        }
    }
}

/** How a picture is fitted, converted to 1 bit and cropped. */
@Composable
private fun ImageProperties(ed: EditorState, e: Element) {
    val fit = e.raw.optString("fit", "contain")
    Group {
        ChoiceRow(
            stringResource(R.string.image_fit),
            listOf(stringResource(R.string.image_fit_contain), stringResource(R.string.image_fit_cover), stringResource(R.string.image_fit_stretch)),
            IMAGE_FITS.indexOf(fit).coerceAtLeast(0),
        ) { if (IMAGE_FITS[it] != fit) ed.setProps(e.id, "fit" to IMAGE_FITS[it]) }
        RowDivider()
        MenuRow(stringResource(R.string.image_dither), ditherName(e.dither), DITHERS.map { it to ditherName(it) }) { if (it != e.dither) ed.setProps(e.id, "dither" to it) }
        RowDivider()
        SwitchRow(stringResource(R.string.prop_invert), on = e.invert) { ed.setProps(e.id, "invert" to it) }
        RowDivider()
        SliderRow(stringResource(R.string.image_threshold), e.threshold.toFloat(), 0f, 255f, { it.roundToInt().toString() }) { ed.setProps(e.id, "threshold" to it.roundToInt()) }
        RowDivider()
        SliderRow(stringResource(R.string.image_brightness), e.raw.optDouble("brightness", 0.0).toFloat(), -1f, 1f, ::percent) { ed.setProps(e.id, "brightness" to round2(it)) }
        RowDivider()
        SliderRow(stringResource(R.string.image_contrast), e.raw.optDouble("contrast", 0.0).toFloat(), -1f, 1f, ::percent) { ed.setProps(e.id, "contrast" to round2(it)) }
    }
    val crop = e.raw.optJSONObject("crop")
    val edges = listOf("left", "top", "right", "bottom")
    val values = edges.map { crop?.optDouble(it, if (it == "right" || it == "bottom") 1.0 else 0.0) ?: if (it == "right" || it == "bottom") 1.0 else 0.0 }
    fun set(i: Int, v: Float) {
        val next = values.toMutableList()
        next[i] = round2(v)
        // Each edge keeps at least 5 % of the picture between it and the opposite edge.
        if (i < 2) next[i] = next[i].coerceIn(0.0, next[i + 2] - 0.05) else next[i] = next[i].coerceIn(next[i - 2] + 0.05, 1.0)
        ed.setRaw(e.id, JSONObject().put("crop", JSONObject().put("left", next[0]).put("top", next[1]).put("right", next[2]).put("bottom", next[3])))
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.image_crop), style = Type.bodyStrong.copy(color = Palette.text2), modifier = Modifier.weight(1f))
        Text(
            stringResource(R.string.image_crop_reset),
            style = Type.bodyStrong.copy(color = if (crop == null) Palette.muted else Palette.primary),
            modifier = Modifier.clickable(enabled = crop != null) { ed.setRaw(e.id, JSONObject().put("crop", JSONObject.NULL)) }.padding(4.dp),
        )
    }
    Group {
        val names = listOf(R.string.crop_left, R.string.crop_top, R.string.crop_right, R.string.crop_bottom)
        edges.indices.forEach { i ->
            if (i > 0) RowDivider()
            SliderRow(stringResource(names[i]), values[i].toFloat(), 0f, 1f, ::percent) { set(i, it) }
        }
    }
}

private fun percent(v: Float): String = "${(v * 100).roundToInt()}%"

private fun round2(v: Float): Double = (v * 100).roundToInt() / 100.0
