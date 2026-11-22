package org.labely.app.editor

import androidx.compose.ui.geometry.Offset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The editor's arithmetic: zoom clamping, handle geometry, export names, check digits, bindings. */
class EditorLogicTest {
    // A 300 × 500 px view with a 50 × 30 mm label fitted at 5 px/mm (250 × 150 px), 25 px padding.
    private val fit = CanvasFit.of(300f, 500f, 50.0, 30.0, 25f)

    @Test
    fun fitsTheLabelIntoTheView() {
        assertEquals(250f, fit.labelW, 0.001f)
        assertEquals(150f, fit.labelH, 0.001f)
        assertEquals(5f, fit.pxPerMm, 0.001f)
        assertEquals(Offset(25f, 175f), fit.origin(Zoom()))
    }

    @Test
    fun zoomStaysBetweenFitAndEightTimes() {
        assertEquals(Zoom(), fit.clamp(Zoom(0.3f, Offset(40f, 40f))))
        assertEquals(8f, fit.clamp(Zoom(20f)).scale)
        assertEquals(8f, fit.transform(Zoom(4f), Offset(150f, 250f), Offset.Zero, 5f).scale)
        assertEquals(1f, fit.transform(Zoom(2f), Offset(150f, 250f), Offset.Zero, 0.1f).scale)
        assertEquals(128, Zoom(1.28f).percent)
    }

    @Test
    fun panKeepsTheLabelInTheView() {
        // At 2× the label is 500 × 300: wider than the view, lower than it.
        val z = fit.clamp(Zoom(2f, Offset(9999f, -9999f)))
        assertEquals((500f - 300f) / 2 + 25f, z.pan.x, 0.001f)
        assertEquals(-(500f - 300f) / 2, z.pan.y, 0.001f)
        // The label still overlaps the view.
        val o = fit.origin(z)
        assertTrue(o.x < 300f && o.x + 500f > 0f && o.y < 500f && o.y + 300f > 0f)
        // At fit there is nothing to pan.
        assertEquals(Offset.Zero, fit.clamp(Zoom(1f, Offset(50f, 50f))).pan)
    }

    @Test
    fun pinchKeepsThePointUnderTheFingers() {
        val finger = Offset(100f, 220f)
        val before = fit.toMm(Zoom(), finger)
        val z = fit.transform(Zoom(), finger, Offset.Zero, 1.5f)
        val after = fit.toMm(z, finger)
        assertEquals(before.x, after.x, 0.01f)
        assertEquals(before.y, after.y, 0.01f)
    }

    @Test
    fun pointsConvertWithZoomAndPan() {
        val z = Zoom(2f, Offset(30f, -20f))
        val p = fit.toPx(z, 10.0, 5.0)
        // 10 px/mm at 2×; origin = centre + pan − half of the scaled label.
        assertEquals(Offset(150f + 30f - 250f + 100f, 250f - 20f - 150f + 50f), p)
        val mm = fit.toMm(z, p)
        assertEquals(10f, mm.x, 0.001f)
        assertEquals(5f, mm.y, 0.001f)
    }

    @Test
    fun handlesResizeFromTheirSide() {
        val f = Frame(10.0, 5.0, 20.0, 10.0)
        assertEquals(Frame(10.0, 5.0, 23.3, 8.0), f.resized(Handle.BottomRight, 3.26, -2.04))
        assertEquals(Frame(12.5, 3.0, 17.5, 12.0), f.resized(Handle.TopLeft, 2.5, -2.0))
        assertEquals(Frame(10.0, 5.0, 20.0, 14.0), f.resized(Handle.Bottom, 99.0, 4.0))
        assertEquals(Frame(6.0, 5.0, 24.0, 10.0), f.resized(Handle.Left, -4.0, 99.0))
    }

    @Test
    fun handlesStopAtOneMillimetre() {
        val f = Frame(10.0, 5.0, 20.0, 10.0)
        assertEquals(Frame(10.0, 5.0, 1.0, 1.0), f.resized(Handle.BottomRight, -50.0, -50.0))
        // The opposite edge stays where it was.
        assertEquals(Frame(29.0, 14.0, 1.0, 1.0), f.resized(Handle.TopLeft, 50.0, 50.0))
    }

    @Test
    fun aLineKeepsItsZeroHeight() {
        val line = Frame(5.0, 10.0, 30.0, 0.0)
        assertEquals(Frame(5.0, 10.0, 35.0, 0.0), line.resized(Handle.Right, 5.0, 7.0))
        assertEquals(Frame(34.0, 10.0, 1.0, 0.0), line.resized(Handle.Left, 80.0, 0.0))
    }

    @Test
    fun handleTouchTargetIsThirtyTwoWide() {
        val tl = Offset(100f, 100f)
        val size = Offset(200f, 100f)
        val all = Handle.entries
        assertEquals(Handle.TopLeft, hitHandle(Offset(88f, 110f), tl, size, all, 32f))
        assertEquals(Handle.BottomRight, hitHandle(Offset(310f, 195f), tl, size, all, 32f))
        assertEquals(Handle.Top, hitHandle(Offset(205f, 90f), tl, size, all, 32f))
        assertNull(hitHandle(Offset(200f, 150f), tl, size, all, 32f))
        assertNull(hitHandle(Offset(60f, 100f), tl, size, all, 32f))
        // A small element: the handles reach outwards, its middle still drags it.
        val small = Offset(20f, 20f)
        assertNull(hitHandle(Offset(110f, 110f), tl, small, all, 32f))
        assertEquals(Handle.TopLeft, hitHandle(Offset(80f, 80f), tl, small, all, 32f))
        // A line's end handle.
        assertEquals(Handle.Right, hitHandle(Offset(310f, 108f), tl, Offset(200f, 0f), listOf(Handle.Left, Handle.Right), 32f))
    }

    @Test
    fun exportNames() {
        assertEquals("Price tag", exportBaseName("Price tag"))
        assertEquals("a_b_c", exportBaseName("a/b:c"))
        assertEquals("label", exportBaseName(" . "))
        assertEquals(listOf("Tag.png"), exportFileNames("Tag", ExportFormat.Png, null))
        assertEquals(listOf("Tag-1.png", "Tag-2.png", "Tag-3.png"), exportFileNames("Tag", ExportFormat.Png, listOf(1, 2, 3)))
        assertEquals(listOf("Tag.pdf"), exportFileNames("Tag", ExportFormat.Pdf, listOf(1, 2, 3)))
    }

    @Test
    fun checkDigits() {
        assertEquals(7, gtinCheckDigit("ean13", "5901234123457"))
        assertEquals(7, gtinCheckDigit("ean13", "590123412345"))
        assertEquals(4, gtinCheckDigit("ean8", "96385074"))
        assertEquals(2, gtinCheckDigit("upc_a", "036000291452"))
        assertNull(gtinCheckDigit("code128", "590123412345"))
        assertNull(gtinCheckDigit("ean13", "12AB"))
    }

    @Test
    fun bindings() {
        val columns = listOf("Name", "Price")
        assertEquals("Price", boundColumn("{{Price}}", columns))
        assertEquals("Name", boundColumn("{{now}} {{ Name }} Ft", columns))
        assertNull(boundColumn("{{now}}", columns))
        assertNull(boundColumn("Price", columns))
        assertEquals("649 Ft", unbound("{{Price}} Ft", "Price", "649"))
        assertEquals("$1", unbound("{{Price}}", "Price", "$1"))
    }

    @Test
    fun dataFiles() {
        assertTrue(isSpreadsheet("a.XLSX", ByteArray(0)))
        assertTrue(isSpreadsheet("download", byteArrayOf(0x50, 0x4B, 0x03, 0x04, 0)))
        assertFalse(isSpreadsheet("a.csv", "a;b".toByteArray()))
        assertEquals("Körte", decodeText("Körte".toByteArray()))
        assertEquals("Körte", decodeText("Körte".toByteArray(charset("windows-1250"))))
    }

    @Test
    fun imagesKeepTheirAspectRatio() {
        assertEquals(1024 to 512, scaledSize(4000, 2000))
        assertEquals(300 to 1024, scaledSize(600, 2048))
        assertEquals(640 to 480, scaledSize(640, 480))
        // A 2:1 picture on a 50 × 30 label: 48 × 24, centred.
        assertEquals(Frame(1.0, 3.0, 48.0, 24.0), imageFrame(50.0, 30.0, 1000, 500))
        // A tall one is limited by the height.
        assertEquals(Frame(18.0, 1.0, 14.0, 28.0), imageFrame(50.0, 30.0, 500, 1000))
    }

    @Test
    fun scannedCodesTakeLabelySymbologies() {
        assertEquals(FoundCode("barcode", "ean13", "5901234123457"), scannedCode("EAN_13", "]E0", "5901234123457"))
        assertEquals(FoundCode("barcode", "code128", "ABC-1"), scannedCode("CODE_128", "]C0", "ABC-1"))
        // FNC1 first marks GS1-128; HRI text keeps the (AI) form the engine encodes.
        assertEquals(FoundCode("barcode", "gs1_128", "(01)09501101530003"), scannedCode("CODE_128", "]C1", "(01)09501101530003"))
        assertEquals("itf", scannedCode("ITF_14", null, "15400141288763")?.symbology)
        assertEquals(FoundCode("code2d", "qr", "https://labely.org"), scannedCode("QR_CODE", "]Q1", "https://labely.org"))
        assertEquals("data_matrix", scannedCode("DATA_MATRIX", null, "x")?.symbology)
        assertEquals("pdf417", scannedCode("PDF_417", null, "x")?.symbology)
        assertEquals("aztec", scannedCode("AZTEC", null, "x")?.symbology)
        // Labely prints neither of these, and an empty read is no code.
        assertNull(scannedCode("MAXI_CODE", null, "x"))
        assertNull(scannedCode("MICRO_QR_CODE", null, "x"))
        assertNull(scannedCode("QR_CODE", null, ""))
    }
}
