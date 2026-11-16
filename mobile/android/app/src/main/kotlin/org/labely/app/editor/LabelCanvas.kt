package org.labely.app.editor

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlin.math.min
import kotlin.math.roundToInt
import org.labely.app.engine.Element
import org.labely.app.ui.theme.Palette

/** Extra touch slop around elements, in millimetres, so thin lines can be picked. */
private const val HIT_SLOP_MM = 1.5

/**
 * The label as the printer will print it (the engine's 1-bit raster), fitted to the space, with
 * the selection drawn on top. Tap selects; dragging an element moves it with `element.move`.
 */
@Composable
fun LabelCanvas(ed: EditorState, modifier: Modifier = Modifier) {
    val doc = ed.doc ?: return
    val label = doc.label
    val density = LocalDensity.current
    val current by rememberUpdatedState(ed)
    var drag by remember { mutableStateOf(Offset.Zero) }
    var dragging by remember { mutableStateOf<Element?>(null) }

    BoxWithConstraints(modifier.background(Palette.canvas).padding(24.dp), contentAlignment = Alignment.Center) {
        val ratio = (label.widthMm / label.heightMm).toFloat()
        val w = min(maxWidth.value, maxHeight.value * ratio)
        val h = w / ratio
        val pxPerMm = with(density) { w.dp.toPx() } / label.widthMm.toFloat()

        fun hit(p: Offset): Element? {
            val x = p.x / pxPerMm
            val y = p.y / pxPerMm
            return current.doc?.elements?.lastOrNull { e ->
                !e.hidden &&
                    x in (e.xMm - HIT_SLOP_MM)..(e.xMm + e.widthMm + HIT_SLOP_MM) &&
                    y in (e.yMm - HIT_SLOP_MM)..(e.yMm + e.heightMm + HIT_SLOP_MM)
            }
        }

        Box(
            Modifier.size(w.dp, h.dp)
                .shadow(10.dp, RoundedCornerShape((label.heightMm * 0.04).dp.coerceAtMost(12.dp)))
                .background(Color.White)
                .pointerInput(pxPerMm) { detectTapGestures { current.selectedId = hit(it)?.id } }
                .pointerInput(pxPerMm) {
                    detectDragGestures(
                        onDragStart = { p ->
                            val sel = current.selected
                            val target = sel?.takeIf { s -> hit(p)?.id == s.id || s.containsPx(p, pxPerMm) } ?: hit(p)
                            if (target != null && !target.locked) {
                                current.selectedId = target.id
                                dragging = target
                            }
                        },
                        onDrag = { change, delta ->
                            if (dragging != null) {
                                change.consume()
                                drag += delta
                            }
                        },
                        onDragEnd = {
                            dragging?.let { e ->
                                val x = round1(e.xMm + drag.x / pxPerMm)
                                val y = round1(e.yMm + drag.y / pxPerMm)
                                current.exec("element.move", "id" to e.id, "x_mm" to x, "y_mm" to y)
                            }
                            dragging = null
                            drag = Offset.Zero
                        },
                        onDragCancel = {
                            dragging = null
                            drag = Offset.Zero
                        },
                    )
                },
        ) {
            ed.preview?.let {
                // FilterQuality.None keeps the printer dots sharp when scaled up.
                Image(it, null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds, filterQuality = FilterQuality.None)
            }
            val sel = ed.selected
            if (sel != null) {
                Canvas(Modifier.fillMaxSize()) {
                    val offset = if (dragging?.id == sel.id) drag else Offset.Zero
                    val tl = Offset(sel.xMm.toFloat() * pxPerMm, sel.yMm.toFloat() * pxPerMm) + offset
                    val size = Size(sel.widthMm.toFloat() * pxPerMm, sel.heightMm.toFloat() * pxPerMm)
                    val stroke = 1.5.dp.toPx()
                    drawRect(Palette.primary, tl, size, style = Stroke(stroke))
                    val r = 4.dp.toPx()
                    val xs = listOf(tl.x, tl.x + size.width / 2, tl.x + size.width)
                    val ys = listOf(tl.y, tl.y + size.height / 2, tl.y + size.height)
                    for (x in xs) for (y in ys) {
                        if (x == xs[1] && y == ys[1]) continue
                        drawRect(Color.White, Offset(x - r, y - r), Size(2 * r, 2 * r))
                        drawRect(Palette.primary, Offset(x - r, y - r), Size(2 * r, 2 * r), style = Stroke(stroke))
                    }
                }
            }
        }
    }
}

private fun Element.containsPx(p: Offset, pxPerMm: Float): Boolean = contains((p.x / pxPerMm).toDouble(), (p.y / pxPerMm).toDouble())

private fun round1(v: Double): Double = (v * 10).roundToInt() / 10.0
