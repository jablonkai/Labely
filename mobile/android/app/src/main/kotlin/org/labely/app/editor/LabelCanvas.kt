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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Extra touch slop around elements at 100 %, in millimetres, so thin lines can be picked. */
private const val HIT_SLOP_MM = 1.5

/** The frame an element is being dragged or resized to; the outline follows it. */
private data class Live(val id: Long, val frame: Frame)

private enum class Mode { Undecided, Move, Resize, Transform, Ignore }

/**
 * The label as the printer will print it (the engine's 1-bit raster), fitted to the space, with
 * the selection drawn on top. Tap selects; dragging an element moves it with `element.move`;
 * dragging a handle resizes it in one `element.set_props`. Two fingers zoom (100 %–800 % of the
 * fitted size) and pan; the badge shows the zoom and resets it.
 */
@Composable
fun LabelCanvas(ed: EditorState, modifier: Modifier = Modifier) {
    val doc = ed.doc ?: return
    val label = doc.label
    val density = LocalDensity.current
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

        fun hit(mm: Offset, scale: Float): Element? {
            val slop = HIT_SLOP_MM / scale
            val x = mm.x.toDouble()
            val y = mm.y.toDouble()
            return current.doc?.elements?.lastOrNull { e ->
                !e.hidden &&
                    x in (e.xMm - slop)..(e.xMm + e.widthMm + slop) &&
                    y in (e.yMm - slop)..(e.yMm + e.heightMm + slop)
            }
        }

        Box(
            Modifier.fillMaxSize().pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val state = current
                    val start = currentFit.clamp(zoom)
                    val startMm = currentFit.toMm(start, down.position)
                    val selected = state.selected
                    val handle = selected?.let { s ->
                        val k = currentFit.scaleOf(start)
                        hitHandle(down.position, currentFit.toPx(start, s.xMm, s.yMm), Offset(s.widthMm.toFloat() * k, s.heightMm.toFloat() * k), Handle.of(s), handleTarget)
                    }
                    var mode = Mode.Undecided
                    var target: Element? = null
                    var total = Offset.Zero
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
                            } else if (mode != Mode.Transform && mode != Mode.Ignore) {
                                val change = pressed.first()
                                total += change.positionChange()
                                if (mode == Mode.Undecided && total.getDistance() > viewConfiguration.touchSlop) {
                                    val under = hit(startMm, start.scale)
                                    val grabbed = selected?.takeIf { s -> under?.id == s.id || s.contains(startMm.x.toDouble(), startMm.y.toDouble()) } ?: under
                                    if (handle != null && selected != null) {
                                        mode = Mode.Resize
                                        target = selected
                                    } else if (grabbed != null && !grabbed.locked) {
                                        state.selectedId = grabbed.id
                                        mode = Mode.Move
                                        target = grabbed
                                    } else {
                                        mode = Mode.Ignore
                                    }
                                }
                                val e = target
                                if (e != null) {
                                    change.consume()
                                    val k = currentFit.scaleOf(currentFit.clamp(zoom))
                                    val dx = (total.x / k).toDouble()
                                    val dy = (total.y / k).toDouble()
                                    dragged = Live(
                                        e.id,
                                        if (mode == Mode.Resize && handle != null) e.frame.resized(handle, dx, dy) else e.frame.copy(xMm = e.xMm + dx, yMm = e.yMm + dy),
                                    )
                                    live = dragged
                                }
                            }
                        }
                    } catch (cancelled: kotlinx.coroutines.CancellationException) {
                        live = null
                        throw cancelled
                    }
                    val e = target
                    val end = dragged
                    when {
                        mode == Mode.Undecided -> {
                            val under = hit(startMm, start.scale)
                            // A second tap on a selected code opens its sheet.
                            if (under != null && under.id == state.selectedId && (under.type == "barcode" || under.type == "code2d")) {
                                state.codeSheet = under.id
                            } else {
                                state.selectedId = under?.id
                            }
                        }
                        e == null || end == null -> live = null
                        mode == Mode.Move -> {
                            val moved = end.frame.copy(xMm = round1(end.frame.xMm), yMm = round1(end.frame.yMm))
                            settle(state.exec("element.move", "id" to e.id, "x_mm" to moved.xMm, "y_mm" to moved.yMm), Live(e.id, moved))
                        }
                        mode == Mode.Resize -> if (end.frame != e.frame) settle(state.resize(e.id, end.frame), end) else live = null
                        else -> live = null
                    }
                }
            },
        ) {
            val wDp = with(density) { fit.labelW.toDp() }
            val hDp = with(density) { fit.labelH.toDp() }
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
                    .shadow(10.dp, RoundedCornerShape((label.heightMm * 0.04).dp.coerceAtMost(12.dp)))
                    .background(Color.White),
            ) {
                ed.preview?.let {
                    // FilterQuality.None keeps the printer dots sharp when scaled up.
                    Image(it, null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds, filterQuality = FilterQuality.None)
                }
            }
            val sel = ed.selected
            if (sel != null) {
                // Drawn over the label in screen pixels, so lines and handles keep their size at any zoom.
                Canvas(Modifier.fillMaxSize()) {
                    val z = fit.clamp(zoom)
                    val k = fit.scaleOf(z)
                    val f = live?.takeIf { it.id == sel.id }?.frame ?: sel.frame
                    val tl = fit.toPx(z, f.xMm, f.yMm)
                    val size = Size(f.widthMm.toFloat() * k, f.heightMm.toFloat() * k)
                    val stroke = 1.5.dp.toPx()
                    drawRect(Palette.primary, tl, size, style = Stroke(stroke))
                    val r = 4.dp.toPx()
                    for (h in Handle.of(sel)) {
                        val c = Offset(tl.x + size.width * h.fx - r, tl.y + size.height * h.fy - r)
                        drawRect(Color.White, c, Size(2 * r, 2 * r))
                        drawRect(Palette.primary, c, Size(2 * r, 2 * r), style = Stroke(stroke))
                    }
                }
            }
            ZoomBadge({ fit.clamp(zoom).percent }, Modifier.align(Alignment.TopEnd).padding(12.dp)) { zoom = Zoom() }
        }
    }
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
