package org.labely.app.editor

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.ui.theme.Ic

/** Icon of an element kind. */
fun kindIcon(type: String): Ic = when (type) {
    "text" -> Ic.Text
    "barcode" -> Ic.Barcode
    "code2d" -> Ic.Qr
    "image" -> Ic.Image
    "rect" -> Ic.Rect
    "ellipse" -> Ic.Ellipse
    "line" -> Ic.Line
    "table" -> Ic.Table
    "border" -> Ic.Border
    else -> Ic.Label
}

@Composable
fun kindName(type: String): String = stringResource(
    when (type) {
        "text" -> R.string.kind_text
        "barcode" -> R.string.kind_barcode
        "code2d" -> R.string.kind_code2d
        "image" -> R.string.kind_image
        "rect" -> R.string.kind_rect
        "ellipse" -> R.string.kind_ellipse
        "line" -> R.string.kind_line
        "table" -> R.string.kind_table
        "icon" -> R.string.kind_icon
        "border" -> R.string.kind_border
        else -> R.string.kind_element
    },
)

/** The element's name, else its text or data, else its kind. */
@Composable
fun elementTitle(e: Element): String = e.name ?: (e.text ?: e.data)?.lineSequence()?.firstOrNull()?.takeIf { it.isNotBlank() } ?: kindName(e.type)

/** `EAN-13`, `QR`: the name of a code's symbology. */
fun symbologyName(e: Element): String =
    (if (e.type == "barcode") SYMBOLOGIES_1D else SYMBOLOGIES_2D).firstOrNull { it.first == e.symbology }?.second ?: e.symbology

@Composable
fun ditherName(dither: String): String = stringResource(
    when (dither) {
        "threshold" -> R.string.dither_threshold
        "atkinson" -> R.string.dither_atkinson
        "bayer" -> R.string.dither_bayer
        else -> R.string.dither_floyd_steinberg
    },
)

@Composable
fun mediaName(media: String): String = stringResource(
    when (media) {
        "black_mark" -> R.string.media_black_mark
        "continuous" -> R.string.media_continuous
        "transparent" -> R.string.media_transparent
        else -> R.string.media_gap
    },
)

val MEDIA = listOf("gap", "black_mark", "continuous", "transparent")
