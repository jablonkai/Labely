package org.labely.app.editor

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.ui.mm
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Extra touch slop around elements at 100 %, in millimetres, so thin lines can be picked. */
private const val HIT_SLOP_MM = 1.5

/** What a drag is doing right now; the outlines follow it until the engine has answered. */
private sealed interface Live

/** The selection moved by ([dx], [dy]) mm, with the guide lines it snapped to. */
private data class Moved(val ids: Set<Long>, val dx: Double, val dy: Double, val xs: List<Double> = emptyList(), val ys: List<Double> = emptyList()) : Live

/** One element resized to [frame] or turned to [rotation]. */
private data class Shaped(val id: Long, val frame: Frame, val rotation: Double) : Live

/** The marquee between two label points (mm). */
private data class Marquee(val from: Offset, val to: Offset) : Live {
    val box: Bounds get() = Bounds(minOf(from.x, to.x).toDouble(), minOf(from.y, to.y).toDouble(), maxOf(from.x, to.x).toDouble(), maxOf(from.y, to.y).toDouble())
}

private enum class Mode { Move, Resize, Rotate, Marquee, Transform, Ignore }

private enum class Start { Tap, Drag, LongPress, Transform }

/**
 * The label as the printer will print it (the engine's 1-bit raster), fitted to the space, with
 * the selection drawn on top. Tap selects; a long press starts multi-select, where taps add and
 * a drag on empty paper draws a marquee. Dragging moves the selection (snapping to the label,
 * other elements and the grid), a handle resizes and the round handle above rotates. Two
 * fingers zoom (100 %–800 % of the fitted size) and pan; the badge shows the zoom and resets it.
 */
@Composable
fun LabelCanvas(ed: EditorState, modifier: Modifier = Modifier) {
    val doc = ed.doc ?: return
    val label = doc.label
    val app = ed.app
    val density = LocalDensity.current
    val haptic = LocalHapticFeedback.current
    val current by rememberUpdatedState(ed)
    var zoom by remember(label.widthMm, label.heightMm) { mutableStateOf(Zoom()) }
    var live by remember { mutableStateOf<Live?>(null) }
    // After a drag the outline stays where the finger left it until the engine has answered.
    fun settle(job: kotlinx.coroutines.Job, shown: Live) {
        live = shown
        job.invokeOnCompletion { if (live === shown) live = null }
    }

    BoxWithConstraints(modifier.background(Palette.canvas).clipToBounds()) {
        val fit = remember(constraints.maxWidth, constraints.maxHeight, label.widthMm, label.heightMm, density) {
            CanvasFit.of(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat(), label.widthMm, label.heightMm, with(density) { 24.dp.toPx() })
        }
        val currentFit by rememberUpdatedState(fit)
        val handleTarget = with(density) { HANDLE_TARGET.toPx() }
        val knobDistance = with(density) { KNOB_DISTANCE.toPx() }
        val snapPx = with(density) { 6.dp.toPx() }

        fun hit(mm: Offset, scale: Float): Element? {
            val slop = HIT_SLOP_MM / scale
            return current.doc?.elements?.lastOrNull { e -> !e.hidden && e.frame.hit(e.rotationDeg, mm.x.toDouble(), mm.y.toDouble(), slop) }
        }

        Box(
            Modifier.fillMaxSize().pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val state = current
                    val start = currentFit.clamp(zoom)
                    val k0 = currentFit.scaleOf(start)
                    val startMm = currentFit.toMm(start, down.position)
                    val single = state.selected
                    // The pointer in the selected element's own (unrotated) coordinates, on screen.
                    val local = single?.let { s ->
                        val (lx, ly) = s.frame.toLocal(s.rotationDeg, startMm.x.toDouble(), startMm.y.toDouble())
                        currentFit.toPx(start, lx, ly)
                    }
                    val handle = if (single != null && local != null) {
                        hitHandle(local, currentFit.toPx(start, single.xMm, single.yMm), Offset(single.widthMm.toFloat() * k0, single.heightMm.toFloat() * k0), Handle.of(single), handleTarget)
                    } else {
                        null
                    }
                    val knob = single != null && local != null && !single.locked && !single.hidden &&
                        (local - (currentFit.toPx(start, single.xMm + single.widthMm / 2, single.yMm) - Offset(0f, knobDistance))).getDistance() <= handleTarget / 2
                    val under = hit(startMm, start.scale)

                    var total = Offset.Zero
                    val first = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                        var decided: Start? = null
                        while (decided == null) {
                            val event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            decided = when {
                                pressed.isEmpty() -> Start.Tap
                                pressed.size >= 2 -> Start.Transform
                                else -> {
                                    total += pressed.first().positionChange()
                                    if (total.getDistance() > viewConfiguration.touchSlop) Start.Drag else null
                                }
                            }
                        }
                        decided
                    } ?: Start.LongPress

                    if (first == Start.Tap) {
                        when {
                            state.multiSelect -> if (under != null) state.toggle(under.id) else state.select(emptyList())
                            // A second tap on a selected code or table opens its sheet.
                            under != null && under.id == state.selectedId && (under.type == "barcode" || under.type == "code2d") -> state.codeSheet = under.id
                            under != null && under.id == state.selectedId && under.type == "table" -> state.tableSheet = under.id
                            else -> state.select(listOfNotNull(under?.id))
                        }
                        return@awaitEachGesture
                    }

                    var mode = Mode.Ignore
                    var moving: List<Element> = emptyList()
                    var others: List<Bounds> = emptyList()
                    var box: Bounds? = null
                    when (first) {
                        Start.Transform -> mode = Mode.Transform
                        Start.LongPress -> {
                            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                            state.multiSelect = true
                            if (under != null) {
                                if (under.id !in state.selection) state.select(state.selection + under.id)
                                mode = Mode.Move
                            } else {
                                mode = Mode.Marquee
                            }
                        }
                        else -> {
                            val grabbed = state.selectedElements.lastOrNull { it.frame.hit(it.rotationDeg, startMm.x.toDouble(), startMm.y.toDouble()) } ?: under
                            when {
                                knob -> mode = Mode.Rotate
                                handle != null && single != null -> mode = Mode.Resize
                                grabbed != null && !grabbed.locked -> {
                                    if (grabbed.id !in state.selection) state.select(if (state.multiSelect) state.selection + grabbed.id else listOf(grabbed.id))
                                    mode = Mode.Move
                                }
                                state.multiSelect -> mode = Mode.Marquee
                            }
                        }
                    }
                    if (mode == Mode.Move) {
                        moving = state.selectedElements.filter { !it.locked }
                        box = boundsOf(moving)
                        val ids = moving.map { it.id }.toSet()
                        others = state.doc?.elements.orEmpty().filter { !it.hidden && it.id !in ids }.map { it.bounds }
                        if (box == null) mode = Mode.Ignore
                    }
                    val base = state.selection

                    var dragged: Live? = null
                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            if (pressed.isEmpty()) break
                            if (pressed.size >= 2) {
                                // A second finger turns any drag into zoom and pan.
                                mode = Mode.Transform
                                dragged = null
                                live = null
                                zoom = currentFit.transform(currentFit.clamp(zoom), event.calculateCentroid(useCurrent = false), event.calculatePan(), event.calculateZoom())
                                event.changes.forEach { it.consume() }
                                continue
                            }
                            if (mode == Mode.Transform || mode == Mode.Ignore) continue
                            val change = pressed.first()
                            total += change.positionChange()
                            change.consume()
                            val z = currentFit.clamp(zoom)
                            val k = currentFit.scaleOf(z)
                            val dx = (total.x / k).toDouble()
                            val dy = (total.y / k).toDouble()
                            val b = box
                            dragged = when {
                                mode == Mode.Move && b != null -> {
                                    val s = if (app.snap) {
                                        snap(label.widthMm, label.heightMm, others, b, dx, dy, (snapPx / k).toDouble(), app.gridMm.takeIf { app.showGrid })
                                    } else {
                                        Snapped(dx, dy)
                                    }
                                    Moved(moving.map { it.id }.toSet(), s.dx, s.dy, s.xs, s.ys)
                                }
                                mode == Mode.Resize && single != null && handle != null ->
                                    Shaped(single.id, single.frame.resizedRotated(handle, dx, dy, single.rotationDeg), single.rotationDeg)
                                mode == Mode.Rotate && single != null -> {
                                    val p = currentFit.toMm(z, change.position)
                                    val angle = handleAngle(single.xMm + single.widthMm / 2, single.yMm + single.heightMm / 2, p.x.toDouble(), p.y.toDouble())
                                    Shaped(single.id, single.frame, snapAngle(angle, if (app.rotateSnap) ROTATE_STEP else 0.0))
                                }
                                mode == Mode.Marquee -> {
                                    val m = Marquee(startMm, currentFit.toMm(z, change.position))
                                    val hits = marqueeHits(state.doc?.elements.orEmpty(), m.box)
                                    state.select(base + hits.filter { it !in base })
                                    m
                                }
                                else -> null
                            }
                            live = dragged
                        }
                    } catch (cancelled: kotlinx.coroutines.CancellationException) {
                        live = null
                        throw cancelled
                    }
                    when (val end = dragged) {
                        is Moved -> if (abs(end.dx) >= 0.05 || abs(end.dy) >= 0.05) settle(state.moveBy(end.dx, end.dy, end.ids.toList()), end) else live = null
                        is Shaped -> {
                            val e = single
                            when {
                                e == null -> live = null
                                mode == Mode.Rotate -> if (end.rotation != e.rotationDeg) settle(state.rotate(e.id, end.rotation), end) else live = null
                                end.frame != e.frame -> settle(state.resize(e.id, end.frame), end)
                                else -> live = null
                            }
                        }
                        else -> {
                            live = null
                            if (mode == Mode.Marquee && state.selection.isEmpty()) state.multiSelect = false
                        }
                    }
                }
            },
        ) {
            val wDp = with(density) { fit.labelW.toDp() }
            val hDp = with(density) { fit.labelH.toDp() }
            val corner = RoundedCornerShape(with(density) { (label.cornerRadiusMm * fit.pxPerMm).toFloat().toDp() })
            val paper = Color(app.labelColor)
            Box(
                Modifier.align(Alignment.Center).size(wDp, hDp)
                    // Zoom and pan are read in the layer, so a pinch does not recompose.
                    .graphicsLayer {
                        val z = fit.clamp(zoom)
                        scaleX = z.scale
                        scaleY = z.scale
                        translationX = z.pan.x
                        translationY = z.pan.y
                    }
                    .shadow(10.dp, corner)
                    .clip(corner)
                    .background(paper),
            ) {
                ed.preview?.let {
                    // FilterQuality.None keeps the printer dots sharp when scaled up. The raster is
                    // black on white; multiplying shows it on the chosen paper colour.
                    Image(
                        it, null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds, filterQuality = FilterQuality.None,
                        colorFilter = if (paper == Color.White) null else ColorFilter.tint(paper, BlendMode.Multiply),
                    )
                }
            }
            val head = app.model(label.printerModel)?.headMm ?: 0.0
            val span = printableSpan(label.widthMm, head)
            val selected = ed.selectedElements
            val single = ed.selected
            val showGrid = app.showGrid
            val showRulers = app.showRulers
            val gridMm = app.gridMm
            // Drawn over the label in screen pixels, so lines and handles keep their size at any zoom.
            Canvas(Modifier.fillMaxSize()) {
                val z = fit.clamp(zoom)
                val k = fit.scaleOf(z)
                val o = fit.origin(z)
                val w = label.widthMm.toFloat() * k
                val h = label.heightMm.toFloat() * k
                val stroke = 1.5.dp.toPx()
                if (span != null) {
                    // What the printhead cannot reach, as the studio shades it.
                    val shade = Palette.red.copy(alpha = 0.16f)
                    drawRect(shade, o, Size(span.first.toFloat() * k, h))
                    drawRect(shade, Offset(o.x + span.second.toFloat() * k, o.y), Size(w - span.second.toFloat() * k, h))
                }
                if (showGrid && gridMm * k >= 4.dp.toPx()) {
                    val c = Palette.text2.copy(alpha = 0.14f)
                    for (x in gridLines(label.widthMm, gridMm)) drawLine(c, Offset(o.x + x.toFloat() * k, o.y), Offset(o.x + x.toFloat() * k, o.y + h))
                    for (y in gridLines(label.heightMm, gridMm)) drawLine(c, Offset(o.x, o.y + y.toFloat() * k), Offset(o.x + w, o.y + y.toFloat() * k))
                }
                if (showRulers) rulers(o, w, h, k, label.widthMm, label.heightMm)

                val l = live
                if (l is Moved) {
                    val guide = Palette.red
                    for (x in l.xs) drawLine(guide, Offset(o.x + x.toFloat() * k, o.y - 8.dp.toPx()), Offset(o.x + x.toFloat() * k, o.y + h + 8.dp.toPx()), 1.dp.toPx())
                    for (y in l.ys) drawLine(guide, Offset(o.x - 8.dp.toPx(), o.y + y.toFloat() * k), Offset(o.x + w + 8.dp.toPx(), o.y + y.toFloat() * k), 1.dp.toPx())
                }
                for (e in selected) {
                    var f = e.frame
                    var rot = e.rotationDeg
                    when {
                        l is Moved && e.id in l.ids -> f = f.copy(xMm = f.xMm + l.dx, yMm = f.yMm + l.dy)
                        l is Shaped && l.id == e.id -> {
                            f = l.frame
                            rot = l.rotation
                        }
                    }
                    val tl = fit.toPx(z, f.xMm, f.yMm)
                    val size = Size(f.widthMm.toFloat() * k, f.heightMm.toFloat() * k)
                    val centre = Offset(tl.x + size.width / 2, tl.y + size.height / 2)
                    rotate(rot.toFloat(), centre) {
                        drawRect(
                            Palette.primary, tl, size,
                            style = Stroke(stroke, pathEffect = if (e.locked) PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx())) else null),
                        )
                        if (e.id == single?.id) {
                            val r = 4.dp.toPx()
                            for (hd in Handle.of(e)) {
                                val c = Offset(tl.x + size.width * hd.fx - r, tl.y + size.height * hd.fy - r)
                                drawRect(Color.White, c, Size(2 * r, 2 * r))
                                drawRect(Palette.primary, c, Size(2 * r, 2 * r), style = Stroke(stroke))
                            }
                            if (!e.locked && !e.hidden) {
                                // The rotation handle: a round knob on a stem above the top edge.
                                val top = Offset(centre.x, tl.y)
                                val knob = top - Offset(0f, KNOB_DISTANCE.toPx())
                                drawLine(Palette.primary, top, knob, stroke)
                                drawCircle(Color.White, 5.dp.toPx(), knob)
                                drawCircle(Palette.primary, 5.dp.toPx(), knob, style = Stroke(stroke))
                            }
                        }
                    }
                }
                if (l is Marquee) {
                    val a = fit.toPx(z, l.box.left, l.box.top)
                    val size = Size(l.box.width.toFloat() * k, l.box.height.toFloat() * k)
                    drawRect(Palette.primary.copy(alpha = 0.12f), a, size)
                    drawRect(Palette.primary, a, size, style = Stroke(1.dp.toPx()))
                }
            }
            (live as? Shaped)?.let { s ->
                if (s.rotation != single?.rotationDeg) {
                    Text(
                        stringResource(R.string.degrees, mm(s.rotation)),
                        style = Type.bodyStrong.copy(fontSize = 13.sp, color = Color.White),
                        modifier = Modifier.align(Alignment.TopCenter).padding(12.dp).clip(RoundedCornerShape(50)).background(Palette.text).padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
            }
            if (ed.multiSelect) {
                Row(
                    Modifier.align(Alignment.TopStart).padding(12.dp).clip(RoundedCornerShape(50)).background(Palette.primary)
                        .clickable { ed.select(emptyList()) }.padding(start = 12.dp, end = 8.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Text(pluralStringResource(R.plurals.selected_count, selected.size, selected.size), style = Type.bodyStrong.copy(fontSize = 13.sp, color = Color.White))
                    LIcon(Ic.Close, Color.White, 14.dp)
                }
            }
            ZoomBadge({ fit.clamp(zoom).percent }, Modifier.align(Alignment.TopEnd).padding(12.dp)) { zoom = Zoom() }
        }
    }
}

/** Millimetre ticks along the label's top and left edges; longer every 5 and 10 mm. */
private fun DrawScope.rulers(o: Offset, w: Float, h: Float, k: Float, widthMm: Double, heightMm: Double) {
    val c = Palette.text2.copy(alpha = 0.6f)
    val gap = 3.dp.toPx()
    val step = if (k >= 3.dp.toPx()) 1 else 5
    fun len(i: Int) = (if (i % 10 == 0) 8.dp else if (i % 5 == 0) 5.dp else 3.dp).toPx()
    var i = 0
    while (i <= widthMm) {
        val x = o.x + i * k
        drawLine(c, Offset(x, o.y - gap), Offset(x, o.y - gap - len(i)))
        i += step
    }
    i = 0
    while (i <= heightMm) {
        val y = o.y + i * k
        drawLine(c, Offset(o.x - gap, y), Offset(o.x - gap - len(i), y))
        i += step
    }
    drawLine(c, Offset(o.x, o.y - gap), Offset(o.x + w, o.y - gap))
    drawLine(c, Offset(o.x - gap, o.y), Offset(o.x - gap, o.y + h))
}

/** The zoom in percent of the fitted size, as in M2; a tap goes back to fit. */
@Composable
private fun ZoomBadge(percent: () -> Int, modifier: Modifier, onReset: () -> Unit) {
    Text(
        stringResource(R.string.zoom_percent, percent()),
        style = Type.bodyStrong.copy(fontSize = 13.sp, color = Palette.text2),
        modifier = modifier.clip(RoundedCornerShape(50)).background(Palette.surface).clickable(onClick = onReset).padding(horizontal = 12.dp, vertical = 6.dp),
    )
}

/** Touch target of a resize handle. */
private val HANDLE_TARGET = 32.dp

/** How far above the top edge the rotation handle sits. */
private val KNOB_DISTANCE = 26.dp

/** Rotation snaps to this many degrees when the option is on. */
const val ROTATE_STEP = 15.0
