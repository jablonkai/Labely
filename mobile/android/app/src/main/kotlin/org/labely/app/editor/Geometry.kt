package org.labely.app.editor

import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.roundToInt
import kotlin.math.roundToLong
import kotlin.math.sin
import org.labely.app.engine.Element

/** Selection, rotation and snapping arithmetic of the canvas, free of UI and engine. */

/** An axis-aligned box in millimetres. */
data class Bounds(val left: Double, val top: Double, val right: Double, val bottom: Double) {
    val width: Double get() = right - left
    val height: Double get() = bottom - top
    val centerX: Double get() = (left + right) / 2
    val centerY: Double get() = (top + bottom) / 2

    fun union(o: Bounds) = Bounds(minOf(left, o.left), minOf(top, o.top), maxOf(right, o.right), maxOf(bottom, o.bottom))

    fun intersects(o: Bounds): Boolean = left <= o.right && o.left <= right && top <= o.bottom && o.top <= bottom
}

/** The box around this frame turned by [rotationDeg] about its centre (the engine's `Element::bounds`). */
fun Frame.bounds(rotationDeg: Double = 0.0): Bounds {
    val r = Math.toRadians(rotationDeg)
    val hw = widthMm / 2
    val hh = heightMm / 2
    val ex = abs(hw * cos(r)) + abs(hh * sin(r))
    val ey = abs(hw * sin(r)) + abs(hh * cos(r))
    val cx = xMm + hw
    val cy = yMm + hh
    return Bounds(cx - ex, cy - ey, cx + ex, cy + ey)
}

val Element.bounds: Bounds get() = frame.bounds(rotationDeg)

/** The box around all of [elements], or null for none. */
fun boundsOf(elements: List<Element>): Bounds? = elements.map { it.bounds }.reduceOrNull(Bounds::union)

/** An angle in 0 until 360 degrees. */
fun normDeg(deg: Double): Double = ((deg % 360.0) + 360.0) % 360.0

/** [deg] rounded to a multiple of [stepDeg], in 0 until 360; unchanged for a step of 0. */
fun snapAngle(deg: Double, stepDeg: Double): Double =
    normDeg(if (stepDeg > 0) (deg / stepDeg).roundToLong() * stepDeg else round1(deg))

/**
 * The rotation of an element whose rotation handle is dragged to ([px], [py]): the handle
 * sits above the centre ([cx], [cy]), so straight up is 0° and clockwise is positive.
 */
fun handleAngle(cx: Double, cy: Double, px: Double, py: Double): Double =
    normDeg(Math.toDegrees(atan2(py - cy, px - cx)) + 90.0)

/** ([x], [y]) turned by [deg] clockwise about the origin (y points down). */
fun rotated(x: Double, y: Double, deg: Double): Pair<Double, Double> {
    val r = Math.toRadians(deg)
    return (x * cos(r) - y * sin(r)) to (x * sin(r) + y * cos(r))
}

/** A label point in the unrotated coordinates of a frame turned by [rotationDeg] about its centre. */
fun Frame.toLocal(rotationDeg: Double, x: Double, y: Double): Pair<Double, Double> {
    if (rotationDeg == 0.0) return x to y
    val cx = xMm + widthMm / 2
    val cy = yMm + heightMm / 2
    val (lx, ly) = rotated(x - cx, y - cy, -rotationDeg)
    return (cx + lx) to (cy + ly)
}

/** Whether ([x], [y]) is on the frame turned by [rotationDeg], with [slop] mm around it. */
fun Frame.hit(rotationDeg: Double, x: Double, y: Double, slop: Double = 0.0): Boolean {
    val (lx, ly) = toLocal(rotationDeg, x, y)
    return lx in (xMm - slop)..(xMm + widthMm + slop) && ly in (yMm - slop)..(yMm + heightMm + slop)
}

/**
 * [resized] for a frame turned by [rotationDeg]: the drag ([dxMm], [dyMm]) is taken along the
 * element's own axes and the point opposite the handle stays where it is on the label.
 */
fun Frame.resizedRotated(handle: Handle, dxMm: Double, dyMm: Double, rotationDeg: Double): Frame {
    if (rotationDeg == 0.0) return resized(handle, dxMm, dyMm)
    val (ldx, ldy) = rotated(dxMm, dyMm, -rotationDeg)
    val sized = resized(handle, ldx, ldy)
    // The anchor: the opposite corner, or the middle of the opposite edge.
    val ax = 1f - handle.fx
    val ay = 1f - handle.fy
    val cx = xMm + widthMm / 2
    val cy = yMm + heightMm / 2
    val (ox, oy) = rotated((ax - 0.5) * widthMm, (ay - 0.5) * heightMm, rotationDeg)
    val (nx, ny) = rotated((ax - 0.5) * sized.widthMm, (ay - 0.5) * sized.heightMm, rotationDeg)
    val ncx = cx + ox - nx
    val ncy = cy + oy - ny
    return Frame(round1(ncx - sized.widthMm / 2), round1(ncy - sized.heightMm / 2), sized.widthMm, sized.heightMm)
}

// ---- snapping ----

/** A snapped drag offset and the guide lines (label millimetres) it snapped to. */
data class Snapped(val dx: Double, val dy: Double, val xs: List<Double> = emptyList(), val ys: List<Double> = emptyList())

/** How close an edge or centre must be to a line to snap to it, in screen millimetres at 100 %. */
const val SNAP_MM = 1.0

/**
 * Snap a drag of [moving] by ([dx], [dy]), as the studio does: its edges and centre go to the
 * label's edges and centre and to the edges and centres of [others] within [threshold] mm;
 * otherwise the left or top edge goes to the [grid] when one is given.
 */
fun snap(labelW: Double, labelH: Double, others: List<Bounds>, moving: Bounds, dx: Double, dy: Double, threshold: Double, grid: Double?): Snapped {
    val cx = mutableListOf(0.0, labelW / 2, labelW)
    val cy = mutableListOf(0.0, labelH / 2, labelH)
    for (b in others) {
        cx += listOf(b.left, b.centerX, b.right)
        cy += listOf(b.top, b.centerY, b.bottom)
    }
    fun axis(lo: Double, hi: Double, d: Double, lines: List<Double>): Pair<Double, Double?> {
        val mine = listOf(lo + d, (lo + hi) / 2 + d, hi + d)
        val best = lines.flatMap { c -> mine.map { m -> (c - m) to c } }.filter { abs(it.first) <= threshold }.minByOrNull { abs(it.first) }
        return when {
            best != null -> (d + best.first) to best.second
            grid != null && grid > 0 -> (Math.rint((lo + d) / grid) * grid - lo) to null
            else -> d to null
        }
    }
    val (sx, gx) = axis(moving.left, moving.right, dx, cx)
    val (sy, gy) = axis(moving.top, moving.bottom, dy, cy)
    return Snapped(sx, sy, listOfNotNull(gx), listOfNotNull(gy))
}

/** Grid lines from 0 to [length] every [step] mm, without the label's own edges. */
fun gridLines(length: Double, step: Double): List<Double> {
    if (step <= 0 || length / step > 400) return emptyList()
    return (1..floor(length / step).toInt()).map { it * step }.filter { it < length - 1e-6 }
}

// ---- selection ----

/** [ids] with every member of the groups they belong to, in z-order. */
fun withGroups(ids: Collection<Long>, elements: List<Element>): List<Long> {
    val groups = elements.filter { it.id in ids }.mapNotNull { it.group }.toSet()
    return elements.filter { it.id in ids || (it.group != null && it.group in groups) }.map { it.id }
}

/** Add [id] (and its group) to [selection], or take it out when it is already there. */
fun toggled(selection: List<Long>, id: Long, elements: List<Element>): List<Long> {
    val unit = withGroups(listOf(id), elements)
    return if (id in selection) selection - unit.toSet() else withGroups(selection + unit, elements)
}

/** Visible elements whose bounds touch the marquee [box], with their groups. */
fun marqueeHits(elements: List<Element>, box: Bounds): List<Long> =
    withGroups(elements.filter { !it.hidden && it.bounds.intersects(box) }.map { it.id }, elements)

/** Whether [ids] hold at least one whole group, so "Ungroup" applies. */
fun hasGroup(ids: Collection<Long>, elements: List<Element>): Boolean = elements.any { it.id in ids && it.group != null }

// ---- units ----

/** The unit lengths are shown and typed in; the engine always works in millimetres. */
enum class Units(val id: String, val perMm: Double, val decimals: Int) {
    Mm("mm", 1.0, 1),
    Inch("in", 1 / 25.4, 3),
    ;

    fun fromMm(mm: Double): Double = mm * perMm

    /** A typed value in millimetres, rounded to 0.01 mm so 1 in is 25.4 mm. */
    fun toMm(v: Double): Double = (v / perMm * 100).roundToLong() / 100.0

    /** [mm] in this unit without trailing zeros: `50`, `12.5`, `1.969`. */
    fun format(mm: Double): String {
        val v = fromMm(mm)
        var k = 1.0
        repeat(decimals) { k *= 10 }
        val r = Math.rint(v * k) / k
        if (r == Math.rint(r)) return r.toLong().toString()
        return java.math.BigDecimal(r).setScale(decimals, java.math.RoundingMode.HALF_UP).stripTrailingZeros().toPlainString()
    }

    companion object {
        fun of(id: String?): Units = entries.firstOrNull { it.id == id } ?: Mm
    }
}

// ---- records ----

/**
 * The records a range such as `1,3,5-7` names, in the order given (1-based, at most [count]),
 * or null when it is not a valid range. `all` and an empty string are every record.
 */
fun parseRecords(text: String, count: Int): List<Int>? {
    val t = text.trim()
    if (t.isEmpty() || t.equals("all", ignoreCase = true)) return (1..count).toList()
    val out = mutableListOf<Int>()
    for (part in t.split(',')) {
        val p = part.trim()
        val ends = p.split('-').map { it.trim().toIntOrNull() ?: return null }
        when (ends.size) {
            1 -> out += ends[0]
            2 -> if (ends[0] <= ends[1]) out += ends[0]..ends[1] else return null
            else -> return null
        }
    }
    return out.takeIf { it.isNotEmpty() && it.all { n -> n in 1..count } }
}

/** A range in the form the engine reads: no spaces. */
fun recordsParam(text: String): String = text.filterNot { it.isWhitespace() }.ifEmpty { "all" }

// ---- rich text ----

/** A style range of a text, in Unicode scalar indexes as in the document. */
data class Span(val start: Int, val end: Int, val bold: Boolean? = null, val italic: Boolean? = null, val underline: Boolean? = null, val fontSizePt: Double? = null) {
    val empty: Boolean get() = bold == null && italic == null && underline == null && fontSizePt == null
}

/** A UTF-16 index of [text] (what a text field reports) as a Unicode scalar index. */
fun scalarIndex(text: String, utf16: Int): Int = text.codePointCount(0, utf16.coerceIn(0, text.length))

/**
 * [spans] with [change] applied to [start] until [end]: spans that reach into the range are cut
 * at its edges, the parts inside take the change, and equal neighbours are joined. Parts of the
 * range no span covers get a span of their own.
 */
fun styleRange(spans: List<Span>, start: Int, end: Int, change: (Span) -> Span): List<Span> {
    if (start >= end) return spans
    val cuts = (spans.flatMap { listOf(it.start, it.end) } + listOf(start, end)).distinct().sorted()
    val out = mutableListOf<Span>()
    for ((a, b) in cuts.zipWithNext()) {
        // Later spans win where they overlap, so the pieces are flattened in order.
        var piece = Span(a, b)
        var covered = false
        for (s in spans) {
            if (s.start <= a && b <= s.end) {
                covered = true
                piece = piece.copy(bold = s.bold ?: piece.bold, italic = s.italic ?: piece.italic, underline = s.underline ?: piece.underline, fontSizePt = s.fontSizePt ?: piece.fontSizePt)
            }
        }
        val inside = start <= a && b <= end
        if (inside) piece = change(piece) else if (!covered) continue
        if (piece.empty) continue
        val last = out.lastOrNull()
        if (last != null && last.end == piece.start && last.copy(start = 0, end = 0) == piece.copy(start = 0, end = 0)) {
            out[out.size - 1] = last.copy(end = piece.end)
        } else {
            out += piece
        }
    }
    return out
}

/** Spans after the text changed from [old] to [new] length: ranges past the end are cut off. */
fun clampSpans(spans: List<Span>, length: Int): List<Span> =
    spans.mapNotNull { s -> if (s.start >= length) null else s.copy(end = minOf(s.end, length)).takeIf { it.end > it.start } }

// ---- tables ----

/** A merged block of table cells, 0-based. */
data class CellRange(val row: Int, val col: Int, val rows: Int, val cols: Int) {
    fun contains(r: Int, c: Int): Boolean = r in row until row + rows && c in col until col + cols
}

/** Weights as percentages of their sum, for display: `[1, 1, 2]` is `[25, 25, 50]`. */
fun percents(weights: List<Double>): List<Int> {
    val sum = weights.sum()
    return if (sum <= 0) weights.map { 0 } else weights.map { (it / sum * 100).roundToInt() }
}

// ---- printhead ----

/**
 * The band across the label width the printhead covers, in mm, when the head ([headMm] wide,
 * centred) is narrower than the label; null when everything prints. Same rule as `doc.check`.
 */
fun printableSpan(labelW: Double, headMm: Double): Pair<Double, Double>? {
    if (headMm <= 0 || labelW <= headMm + 0.01) return null
    val left = (labelW - headMm) / 2
    return left to left + headMm
}

// ---- lines ----

/** Line style of a `dash_mm` value, as the studio reads it: a dash no longer than its gap is dotted. */
enum class LineStyle {
    Solid, Dashed, Dotted;

    /** The `dash_mm` pair for this style, following the stroke width; null is solid. */
    fun dash(strokeMm: Double): Pair<Double, Double>? {
        val s = maxOf(strokeMm, 0.1)
        return when (this) {
            Solid -> null
            Dashed -> maxOf(s * 6, 1.5) to maxOf(s * 3, 0.8)
            Dotted -> s to s * 1.5
        }
    }

    companion object {
        fun of(on: Double?, off: Double?): LineStyle = if (on == null || off == null) Solid else if (on <= off) Dotted else Dashed
    }
}

// ---- printers ----

private val SCHEMES = setOf("ble", "usb", "tcp", "sim", "fake")

/** What the user typed as a printer address: a value without a known scheme is a network host. */
fun manualAddress(typed: String): String {
    val t = typed.trim()
    return if (':' in t && t.substringBefore(':') in SCHEMES) t else "tcp:$t"
}
