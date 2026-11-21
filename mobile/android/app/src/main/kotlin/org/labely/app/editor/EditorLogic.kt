package org.labely.app.editor

import androidx.compose.ui.geometry.Offset
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import org.labely.app.engine.Element

/** The editor's arithmetic, free of UI and engine so it can be unit-tested. */

/** An element frame in millimetres. */
data class Frame(val xMm: Double, val yMm: Double, val widthMm: Double, val heightMm: Double)

val Element.frame: Frame get() = Frame(xMm, yMm, widthMm, heightMm)

/** Positions are sent to the engine in tenths of a millimetre. */
fun round1(v: Double): Double = (v * 10).roundToInt() / 10.0

// ---- zoom ----

/** Zoom of the canvas: [scale] times the fitted size, moved by [pan] pixels from the centre. */
data class Zoom(val scale: Float = 1f, val pan: Offset = Offset.Zero) {
    val percent: Int get() = (scale * 100).roundToInt()
}

/**
 * The canvas area ([viewW] × [viewH] px) and the label fitted into it ([labelW] × [labelH] px
 * at 100 %). Converts between screen pixels and label millimetres and keeps a [Zoom] in range.
 */
data class CanvasFit(val viewW: Float, val viewH: Float, val labelW: Float, val labelH: Float, val pxPerMm: Float, val slack: Float = 0f) {
    /** Scale within 100 %–800 %, and the pan limited so the label cannot leave the view. */
    fun clamp(z: Zoom): Zoom {
        val s = z.scale.coerceIn(MIN_ZOOM, MAX_ZOOM)
        if (s == MIN_ZOOM) return Zoom()
        val mx = panLimit(viewW, labelW * s)
        val my = panLimit(viewH, labelH * s)
        return Zoom(s, Offset(z.pan.x.coerceIn(-mx, mx), z.pan.y.coerceIn(-my, my)))
    }

    /** A label smaller than the view stays inside it; a larger one keeps covering it, plus [slack]. */
    private fun panLimit(view: Float, label: Float): Float = if (label <= view) (view - label) / 2 else (label - view) / 2 + slack

    /** Pinch by [factor] around [centroid] (the point under the fingers stays there) and move by [pan]. */
    fun transform(z: Zoom, centroid: Offset, pan: Offset, factor: Float): Zoom {
        val s = (z.scale * factor).coerceIn(MIN_ZOOM, MAX_ZOOM)
        val fromCentre = centroid - Offset(viewW / 2, viewH / 2)
        val moved = fromCentre - (fromCentre - z.pan) * (s / z.scale) + pan
        return clamp(Zoom(s, moved))
    }

    /** Top-left corner of the label on screen. */
    fun origin(z: Zoom): Offset = Offset(viewW / 2 + z.pan.x - labelW * z.scale / 2, viewH / 2 + z.pan.y - labelH * z.scale / 2)

    /** Screen pixels per millimetre at [z]. */
    fun scaleOf(z: Zoom): Float = pxPerMm * z.scale

    /** A screen point in label millimetres. */
    fun toMm(z: Zoom, p: Offset): Offset = (p - origin(z)) / scaleOf(z)

    /** A label point (mm) on screen. */
    fun toPx(z: Zoom, xMm: Double, yMm: Double): Offset = origin(z) + Offset(xMm.toFloat(), yMm.toFloat()) * scaleOf(z)

    companion object {
        const val MIN_ZOOM = 1f
        const val MAX_ZOOM = 8f

        /** Fit a [widthMm] × [heightMm] label into the view, leaving [padding] px around it. */
        fun of(viewW: Float, viewH: Float, widthMm: Double, heightMm: Double, padding: Float): CanvasFit {
            val ratio = (widthMm / heightMm).toFloat()
            val w = min(viewW - 2 * padding, (viewH - 2 * padding) * ratio).coerceAtLeast(1f)
            return CanvasFit(viewW, viewH, w, w / ratio, w / widthMm.toFloat(), padding)
        }
    }
}

// ---- resize handles ----

/** A resize handle at ([fx], [fy]) of the frame: 0 is the left or top edge, 1 the right or bottom. */
enum class Handle(val fx: Float, val fy: Float) {
    TopLeft(0f, 0f), Top(0.5f, 0f), TopRight(1f, 0f),
    Left(0f, 0.5f), Right(1f, 0.5f),
    BottomLeft(0f, 1f), Bottom(0.5f, 1f), BottomRight(1f, 1f),
    ;

    companion object {
        /** Eight handles; a line gets its two ends; a locked or hidden element none. */
        fun of(e: Element): List<Handle> = when {
            e.locked || e.hidden -> emptyList()
            e.type == "line" && e.heightMm == 0.0 -> listOf(Left, Right)
            e.type == "line" && e.widthMm == 0.0 -> listOf(Top, Bottom)
            e.type == "line" -> listOf(TopLeft, BottomRight)
            else -> entries
        }
    }
}

/** Smallest side a handle can drag an element to, in millimetres. */
const val MIN_SIZE_MM = 1.0

/**
 * The frame after dragging [handle] by ([dxMm], [dyMm]): the opposite edge stays, no side gets
 * under [MIN_SIZE_MM], values are rounded to 0.1 mm. A side of zero (a line) stays zero.
 */
fun Frame.resized(handle: Handle, dxMm: Double, dyMm: Double): Frame {
    var left = xMm
    var top = yMm
    var right = xMm + widthMm
    var bottom = yMm + heightMm
    if (widthMm > 0) {
        if (handle.fx == 0f) left = min(left + dxMm, right - MIN_SIZE_MM)
        if (handle.fx == 1f) right = max(right + dxMm, left + MIN_SIZE_MM)
    }
    if (heightMm > 0) {
        if (handle.fy == 0f) top = min(top + dyMm, bottom - MIN_SIZE_MM)
        if (handle.fy == 1f) bottom = max(bottom + dyMm, top + MIN_SIZE_MM)
    }
    val side = { v: Double, old: Double -> if (old > 0) round1(v).coerceAtLeast(min(MIN_SIZE_MM, old)) else 0.0 }
    return Frame(round1(left), round1(top), side(right - left, widthMm), side(bottom - top, heightMm))
}

/**
 * The handle under [p]. The frame is at [topLeft] with [size], in pixels. Each handle has a
 * [target] px wide touch area; on a small element it moves outwards, so the middle still drags.
 */
fun hitHandle(p: Offset, topLeft: Offset, size: Offset, handles: List<Handle>, target: Float): Handle? {
    fun distance(f: Float, v: Float, start: Float, side: Float): Float? {
        val inside = if (side < 1f) target / 2 else min(target / 2, side / 4)
        val (lo, hi) = when (f) {
            0f -> (start - (target - inside)) to (start + inside)
            1f -> (start + side - inside) to (start + side + (target - inside))
            else -> (start + side / 2 - inside) to (start + side / 2 + inside)
        }
        return if (v in lo..hi) abs(v - (start + side * f)) else null
    }
    return handles.mapNotNull { h ->
        val dx = distance(h.fx, p.x, topLeft.x, size.x) ?: return@mapNotNull null
        val dy = distance(h.fy, p.y, topLeft.y, size.y) ?: return@mapNotNull null
        h to dx + dy
    }.minByOrNull { it.second }?.first
}

// ---- images ----

/** [w] × [h] scaled down so the longer side is at most [max]; never scaled up. */
fun scaledSize(w: Int, h: Int, max: Int = MAX_IMAGE_PX): Pair<Int, Int> {
    val longer = max(w, h)
    if (longer <= max) return w to h
    val k = max.toDouble() / longer
    return (w * k).roundToInt().coerceAtLeast(1) to (h * k).roundToInt().coerceAtLeast(1)
}

const val MAX_IMAGE_PX = 1024

/** 1-bit conversions of an image (`dither`), in the engine's order. */
val DITHERS = listOf("threshold", "floyd_steinberg", "atkinson", "bayer")

/** A centred frame with the picture's aspect ratio, inside the label with a 1 mm margin. */
fun imageFrame(labelW: Double, labelH: Double, widthPx: Int, heightPx: Int): Frame {
    val maxW = (labelW - 2).coerceAtLeast(MIN_SIZE_MM)
    val maxH = (labelH - 2).coerceAtLeast(MIN_SIZE_MM)
    val k = min(maxW / widthPx, maxH / heightPx)
    val w = round1(widthPx * k).coerceAtLeast(0.1)
    val h = round1(heightPx * k).coerceAtLeast(0.1)
    return Frame(round1((labelW - w) / 2), round1((labelH - h) / 2), w, h)
}

// ---- data ----

private val BINDING = Regex("""\{\{([^{}]+)\}\}""")

/** The first of [columns] that [content] uses as `{{column}}`, or null. */
fun boundColumn(content: String?, columns: List<String>): String? =
    content?.let { c -> BINDING.findAll(c).map { it.groupValues[1].trim() }.firstOrNull { it in columns } }

/** [content] with every `{{column}}` replaced by [value]. */
fun unbound(content: String, column: String, value: String): String =
    Regex("""\{\{\s*""" + Regex.escape(column) + """\s*\}\}""").replace(content) { value }

/** Whether a picked data file is a spreadsheet (xlsx, xls, ods) rather than delimited text. */
fun isSpreadsheet(name: String, bytes: ByteArray): Boolean {
    if (name.substringAfterLast('.', "").lowercase() in SPREADSHEETS) return true
    val magic = bytes.take(4).map { it.toInt() and 0xFF }
    return magic == listOf(0x50, 0x4B, 0x03, 0x04) || magic == listOf(0xD0, 0xCF, 0x11, 0xE0)
}

private val SPREADSHEETS = setOf("xlsx", "xlsm", "xls", "ods")

/** CSV bytes as text: UTF-8, or Windows-1250 (what Excel writes in Central Europe) when that fails. */
fun decodeText(bytes: ByteArray): String =
    try {
        Charsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(bytes)).toString()
    } catch (_: java.nio.charset.CharacterCodingException) {
        String(bytes, runCatching { charset("windows-1250") }.getOrDefault(Charsets.ISO_8859_1))
    }

// ---- export ----

enum class ExportFormat(val ext: String, val mime: String) {
    Png("png", "image/png"),
    Pdf("pdf", "application/pdf"),
}

val EXPORT_DPI = listOf(203, 300, 600)

/** A document name as a file name: path and reserved characters become `_`. */
fun exportBaseName(docName: String): String =
    docName.replace(Regex("""[\\/:*?"<>|\p{Cntrl}]"""), "_").trim().trim('.').ifEmpty { "label" }

/**
 * The files an export writes: one `<name>.<ext>`, or for a PNG of several [records] one
 * `<name>-<record>.png` each. A PDF holds all its records in one file.
 */
fun exportFileNames(docName: String, format: ExportFormat, records: List<Int>?): List<String> {
    val base = exportBaseName(docName)
    return if (format == ExportFormat.Png && records != null) records.map { "$base-$it.png" } else listOf("$base.${format.ext}")
}

// ---- codes ----

val SYMBOLOGIES_1D = listOf(
    "ean13" to "EAN-13", "code128" to "Code 128", "code39" to "Code 39", "ean8" to "EAN-8", "upc_a" to "UPC-A", "upc_e" to "UPC-E",
    "itf" to "ITF", "code93" to "Code 93", "codabar" to "Codabar", "gs1_128" to "GS1-128",
    "code128_a" to "Code 128 A", "code128_b" to "Code 128 B", "code128_c" to "Code 128 C",
)

val SYMBOLOGIES_2D = listOf("qr" to "QR", "data_matrix" to "Data Matrix", "pdf417" to "PDF417", "aztec" to "Aztec")

/**
 * The check digit of EAN-13, EAN-8 or UPC-A [data]: the last digit of a complete number, or the
 * one the printer adds when it is left off. Null for other symbologies and other lengths.
 */
fun gtinCheckDigit(symbology: String, data: String): Int? {
    val len = when (symbology) {
        "ean13" -> 13
        "ean8" -> 8
        "upc_a" -> 12
        else -> return null
    }
    if (data.any { it !in '0'..'9' } || data.length !in (len - 1)..len) return null
    val sum = data.take(len - 1).reversed().withIndex().sumOf { (i, c) -> (c - '0') * if (i % 2 == 0) 3 else 1 }
    return (10 - sum % 10) % 10
}
