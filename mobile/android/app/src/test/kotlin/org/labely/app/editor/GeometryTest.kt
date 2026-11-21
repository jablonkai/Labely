package org.labely.app.editor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.labely.app.engine.Serial

/** Selection, rotation, snapping, units, record ranges and style ranges: the canvas arithmetic. */
class GeometryTest {
    @Test
    fun boundsGrowWithRotation() {
        val f = Frame(10.0, 10.0, 20.0, 10.0)
        assertEquals(Bounds(10.0, 10.0, 30.0, 20.0), f.bounds())
        val turned = f.bounds(90.0)
        assertEquals(15.0, turned.left, 1e-9)
        assertEquals(5.0, turned.top, 1e-9)
        assertEquals(10.0, turned.width, 1e-9)
        assertEquals(20.0, turned.height, 1e-9)
        assertTrue(f.hit(90.0, 20.0, 6.0))
        assertTrue(!f.hit(90.0, 11.0, 15.0))
        assertTrue(f.hit(0.0, 11.0, 15.0))
    }

    @Test
    fun anglesSnapAndFollowTheHandle() {
        assertEquals(350.0, normDeg(-10.0), 1e-9)
        assertEquals(45.0, snapAngle(52.0, 15.0), 1e-9)
        assertEquals(0.0, snapAngle(355.0, 15.0), 1e-9)
        assertEquals(52.3, snapAngle(52.31, 0.0), 1e-9)
        assertEquals(0.0, handleAngle(10.0, 10.0, 10.0, 0.0), 1e-9)
        assertEquals(90.0, handleAngle(10.0, 10.0, 20.0, 10.0), 1e-9)
        assertEquals(270.0, handleAngle(10.0, 10.0, 0.0, 10.0), 1e-9)
    }

    @Test
    fun rotatedResizeKeepsTheOppositeSide() {
        val f = Frame(10.0, 10.0, 20.0, 10.0)
        assertEquals(f.resized(Handle.Right, 4.0, 0.0), f.resizedRotated(Handle.Right, 4.0, 0.0, 0.0))
        // Turned 90°, the right edge points down: dragging down by 4 widens by 4.
        val r = f.resizedRotated(Handle.Right, 0.0, 4.0, 90.0)
        assertEquals(24.0, r.widthMm, 1e-9)
        assertEquals(10.0, r.heightMm, 1e-9)
        // The left edge's middle was at (20, 5) on the label and stays there.
        val (ax, ay) = rotated(-r.widthMm / 2, 0.0, 90.0)
        assertEquals(20.0, r.xMm + r.widthMm / 2 + ax, 0.11)
        assertEquals(5.0, r.yMm + r.heightMm / 2 + ay, 0.11)
    }

    @Test
    fun snapsToLabelThenElementsThenGrid() {
        val moving = Bounds(10.0, 10.0, 20.0, 15.0)
        // Left edge lands 0.4 mm from the label's left edge.
        val edge = snap(50.0, 30.0, emptyList(), moving, -9.6, 0.0, 1.0, null)
        assertEquals(-10.0, edge.dx, 1e-9)
        assertEquals(listOf(0.0), edge.xs)
        // Centre to the label's centre.
        val centre = snap(50.0, 30.0, emptyList(), moving, 10.3, 0.0, 1.0, null)
        assertEquals(10.0, centre.dx, 1e-9)
        assertEquals(listOf(25.0), centre.xs)
        // The other element's right edge at 37.
        val other = snap(50.0, 30.0, listOf(Bounds(30.0, 0.0, 37.0, 4.0)), moving, 16.6, 0.0, 1.0, null)
        assertEquals(17.0, other.dx, 1e-9)
        assertEquals(listOf(37.0), other.xs)
        // Nothing near: the grid takes the left and top edges, without a guide.
        val grid = snap(50.0, 30.0, emptyList(), Bounds(3.0, 3.0, 7.0, 6.0), 3.2, 4.4, 0.1, 2.0)
        assertEquals(3.0, grid.dx, 1e-9)
        assertEquals(5.0, grid.dy, 1e-9)
        assertTrue(grid.xs.isEmpty() && grid.ys.isEmpty())
        // Without a grid the drag is free.
        assertEquals(3.2, snap(50.0, 30.0, emptyList(), Bounds(3.0, 3.0, 7.0, 6.0), 3.2, 4.4, 0.1, null).dx, 1e-9)
        assertEquals(listOf(5.0, 10.0, 15.0), gridLines(20.0, 5.0))
        assertTrue(gridLines(20.0, 0.0).isEmpty())
    }

    @Test
    fun unitsConvertBothWays() {
        assertEquals("50", Units.Mm.format(50.0))
        assertEquals("12.5", Units.Mm.format(12.5))
        assertEquals("1.969", Units.Inch.format(50.0))
        assertEquals("1", Units.Inch.format(25.4))
        assertEquals(25.4, Units.Inch.toMm(1.0), 1e-9)
        assertEquals(12.5, Units.Mm.toMm(12.5), 1e-9)
        assertEquals(Units.Inch, Units.of("in"))
        assertEquals(Units.Mm, Units.of(null))
    }

    @Test
    fun recordRangesAreParsed() {
        assertEquals(listOf(1, 3, 5, 6, 7), parseRecords("1, 3, 5-7", 10))
        assertEquals(listOf(1, 2, 3), parseRecords("", 3))
        assertEquals(listOf(1, 2), parseRecords("ALL", 2))
        assertNull(parseRecords("5-3", 10))
        assertNull(parseRecords("1,11", 10))
        assertNull(parseRecords("a", 10))
        assertNull(parseRecords("1-2-3", 10))
        assertEquals("1,3,5-7", recordsParam("1, 3, 5-7"))
        assertEquals("all", recordsParam("  "))
    }

    @Test
    fun styleRangesAreCutAndJoined() {
        val bold = styleRange(emptyList(), 2, 5) { it.copy(bold = true) }
        assertEquals(listOf(Span(2, 5, bold = true)), bold)
        // Italic over a part of it splits the span.
        val mixed = styleRange(bold, 4, 8) { it.copy(italic = true) }
        assertEquals(listOf(Span(2, 4, bold = true), Span(4, 5, bold = true, italic = true), Span(5, 8, italic = true)), mixed)
        // Clearing the same range joins what is left.
        assertEquals(bold, styleRange(mixed, 4, 8) { it.copy(italic = null) })
        // Extending bold joins equal neighbours.
        assertEquals(listOf(Span(2, 9, bold = true)), styleRange(bold, 5, 9) { it.copy(bold = true) })
        assertEquals(bold, styleRange(bold, 3, 3) { it.copy(bold = null) })
        assertEquals(listOf(Span(2, 4, bold = true)), clampSpans(listOf(Span(2, 5, bold = true), Span(6, 8, italic = true)), 4))
        assertEquals(2, scalarIndex("a😀b", 3))
    }

    @Test
    fun smallHelpers() {
        assertEquals(listOf(25, 25, 50), percents(listOf(1.0, 1.0, 2.0)))
        assertTrue(CellRange(1, 1, 2, 2).contains(2, 2))
        assertTrue(!CellRange(1, 1, 2, 2).contains(3, 1))
        assertNull(printableSpan(40.0, 48.0))
        assertEquals(1.0 to 49.0, printableSpan(50.0, 48.0))
        assertEquals(LineStyle.Solid, LineStyle.of(null, null))
        assertEquals(LineStyle.Dotted, LineStyle.of(0.4, 0.6))
        assertEquals(LineStyle.Dashed, LineStyle.of(2.4, 1.2))
        assertNull(LineStyle.Solid.dash(0.4))
        assertEquals(LineStyle.Dashed, LineStyle.Dashed.dash(0.4)!!.let { LineStyle.of(it.first, it.second) })
        assertEquals(LineStyle.Dotted, LineStyle.Dotted.dash(0.4)!!.let { LineStyle.of(it.first, it.second) })
        assertEquals("tcp:192.168.1.50", manualAddress(" 192.168.1.50 "))
        assertEquals("tcp:printer.local:9100", manualAddress("printer.local:9100"))
        assertEquals("tcp:10.0.0.2", manualAddress("tcp:10.0.0.2"))
        assertEquals("ble:AA:BB", manualAddress("ble:AA:BB"))
        val s = Serial("n", 5, 2, 3, "A-", "/x")
        assertEquals("A-005/x", s.value(0))
        assertEquals("A-009/x", s.value(2))
    }
}
