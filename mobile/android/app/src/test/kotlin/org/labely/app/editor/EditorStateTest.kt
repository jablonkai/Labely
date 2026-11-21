package org.labely.app.editor

import android.content.Context
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import java.time.Duration
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import javax.imageio.ImageIO
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.labely.app.LabelyApplication
import org.labely.app.SIM_ADDRESS
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * The editor's and the printer screen's logic on the real engine (built for the host, loaded
 * through JNA) and the B21 Pro simulator, as in `LabelyFunctionsTest`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class EditorStateTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val app get() = (context as LabelyApplication).state

    private val csv = "Name;Price;EAN\nApple;649;5998001234561\nPear;899;5998001234562\nBanana;749;5998001234563\n"

    // ---- data merge ----

    @Test
    fun attachCsvShowsItsRecords() {
        val ed = editor()
        assertNull(ed.data)
        val plain = ed.preview
        onMain { ed.attachDataNow("products.csv", csv.toByteArray()) }
        assertEquals(DataInfo(null, listOf("Name", "Price", "EAN"), 3), ed.data)
        assertEquals("products.csv", ed.sourceName)
        assertEquals(listOf("Apple", "649", "5998001234561"), ed.rows.first())
        assertEquals(1, ed.record)
        assertNotSame(plain, ed.preview)

        val first = ed.preview
        onMain { ed.showRecord(2).join() }
        assertEquals(2, ed.record)
        assertNotSame(first, ed.preview)
        assertEquals("899", ed.value("Price"))
        onMain { ed.showRecord(99).join() }
        assertEquals(3, ed.record)
        onMain { ed.showRecord(0).join() }
        assertEquals(1, ed.record)

        onMain { ed.detachData().join() }
        assertNull(ed.data)
        assertEquals(0, ed.recordCount)
        assertTrue(ed.rows.isEmpty())
    }

    @Test
    fun rowsAreReadInPages() {
        val ed = editor()
        val big = "n\n" + (1..120).joinToString("\n")
        onMain { ed.attachDataNow("big.csv", big.toByteArray()) }
        assertEquals(120, ed.recordCount)
        assertEquals(EditorState.PAGE, ed.rows.size)
        onMain { ed.loadMoreRows().join() }
        assertEquals(100, ed.rows.size)
        onMain { ed.loadMoreRows().join() }
        onMain { ed.loadMoreRows().join() }
        assertEquals(120, ed.rows.size)
        assertEquals(listOf("120"), ed.rows.last())
    }

    @Test
    fun attachSpreadsheetGoesThroughTheDataDir() {
        val ed = editor()
        onMain { ed.attachDataNow("products.xlsx", xlsx(listOf(listOf("Name", "Price"), listOf("Apple", "649"), listOf("Pear", "899")))) }
        assertEquals(DataInfo("products.xlsx", listOf("Name", "Price"), 2), ed.data)
        assertEquals("products.xlsx", ed.sourceName)
        assertEquals(listOf("Pear", "899"), ed.rows[1])
        // The copy is only needed while the engine reads it.
        assertTrue(app.importsDir.listFiles().orEmpty().isEmpty())
    }

    @Test
    fun bindingIsTheColumnInTheElementsString() {
        val ed = editor()
        onMain {
            ed.attachDataNow("products.csv", csv.toByteArray())
            ed.addText("Text").join()
        }
        val id = ed.selectedId!!
        assertNull(ed.boundColumn(ed.selected!!))
        assertEquals(0, ed.boundCount)

        onMain { ed.bind(id, "Name").join() }
        assertEquals("{{Name}}", ed.selected!!.text)
        assertEquals("Name", ed.boundColumn(ed.selected!!))
        assertEquals(1, ed.boundCount)

        onMain {
            ed.showRecord(2).join()
            ed.unbind(ed.selected!!)!!.join()
        }
        assertEquals("Pear", ed.selected!!.text)
        assertNull(ed.boundColumn(ed.selected!!))
    }

    // ---- export ----

    @Test
    fun exportWritesNamedFilesIntoTheExportsDir() {
        val ed = editor()
        val name = ed.doc!!.name
        val png = onMain { ed.export(ExportFormat.Png, 300, all = false) }.single()
        assertEquals(java.io.File(app.exportsDir, "$name.png"), png)
        assertTrue(png.readBytes().take(4) == listOf<Byte>(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte()))

        val pdf = onMain { ed.export(ExportFormat.Pdf, 203, all = true) }.single()
        assertEquals("$name.pdf", pdf.name)
        assertTrue(pdf.readText(Charsets.ISO_8859_1).startsWith("%PDF"))
        // The folder is emptied before each export.
        assertEquals(listOf("$name.pdf"), app.exportsDir.list()!!.toList())
    }

    @Test
    fun exportOfAllRecordsIsOnePngEachOrOnePdf() {
        val ed = editor()
        val name = ed.doc!!.name
        onMain {
            ed.attachDataNow("products.csv", csv.toByteArray())
            ed.addText("Text").join()
            ed.bind(ed.selectedId!!, "Name").join()
            ed.showRecord(2).join()
        }
        val all = onMain { ed.export(ExportFormat.Png, 203, all = true) }
        assertEquals(listOf("$name-1.png", "$name-2.png", "$name-3.png"), all.map { it.name })
        assertTrue(all.all { it.isFile })
        val second = all[1].readBytes()
        assertFalse(all[0].readBytes().contentEquals(second))

        // Current = the previewed record.
        val current = onMain { ed.export(ExportFormat.Png, 203, all = false) }.single()
        assertEquals("$name.png", current.name)
        assertTrue(current.readBytes().contentEquals(second))

        val one = onMain { ed.export(ExportFormat.Pdf, 203, all = false) }.single().length()
        val three = onMain { ed.export(ExportFormat.Pdf, 203, all = true) }.single().length()
        assertTrue("one page $one, three pages $three", three > one)
    }

    // ---- resize ----

    @Test
    fun resizeIsOneCommandAndOneUndoStep() {
        val ed = editor()
        onMain { ed.addRect().join() }
        val before = ed.selected!!.frame
        val after = before.resized(Handle.TopLeft, 2.0, 1.0)
        onMain { ed.resize(ed.selectedId!!, after).join() }
        assertEquals(after, ed.selected!!.frame)
        assertEquals(Frame(before.xMm + 2, before.yMm + 1, before.widthMm - 2, before.heightMm - 1), after)
        onMain { ed.undo().join() }
        assertEquals(before, ed.doc!!.elements.single().frame)
    }

    @Test
    fun handlesFollowTheElement() {
        val ed = editor()
        onMain { ed.addRect().join() }
        assertEquals(8, Handle.of(ed.selected!!).size)
        onMain { ed.setProps(ed.selectedId!!, "rotation_deg" to 30).join() }
        assertTrue(Handle.of(ed.selected!!).isEmpty())
        onMain { ed.addLine().join() }
        assertEquals(listOf(Handle.Left, Handle.Right), Handle.of(ed.selected!!))
        onMain { ed.exec("element.lock", "ids" to org.json.JSONArray(listOf(ed.selectedId!!))).join() }
        assertTrue(Handle.of(ed.selected!!).isEmpty())
    }

    // ---- barcodes ----

    @Test
    fun insertingACodeOpensItsSheet() {
        val ed = editor()
        onMain { ed.addBarcode().join() }
        assertEquals("barcode", ed.selected!!.type)
        assertEquals(ed.selectedId, ed.codeSheet)
    }

    @Test
    fun aValueTheEngineCannotEncodeIsNotApplied() {
        val ed = editor()
        onMain { ed.addBarcode().join() }
        val e = ed.selected!!
        val bad = onMain { ed.applyCode(e, "ean13", "12AB") }
        assertNotNull(bad.error)
        assertEquals(e.data, ed.selected!!.data)
        assertEquals("code128", ed.selected!!.symbology)

        val good = onMain { ed.applyCode(e, "ean13", "5901234123457") }
        assertEquals(CodeCheck(null, 7), good)
        assertEquals("ean13", ed.selected!!.symbology)
        assertEquals("5901234123457", ed.selected!!.data)

        // One undo step back to the value from before.
        onMain { ed.undo().join() }
        assertEquals(e.data, ed.doc!!.elements.single().data)

        onMain { ed.setProps(e.id, "human_readable" to "above").join() }
        assertEquals("above", ed.doc!!.elements.single().humanReadable)
    }

    @Test
    fun aQrCodeSwitchesAmongTwoDimensionalSymbologies() {
        val ed = editor()
        onMain { ed.addQr().join() }
        val e = ed.selected!!
        assertEquals("qr", e.symbology)
        for ((id, _) in SYMBOLOGIES_2D) {
            assertNull(id, onMain { ed.applyCode(ed.selected!!, id, "LABELY-42") }.error)
            assertEquals(id, ed.selected!!.symbology)
        }
    }

    @Test
    fun everyListedBarcodeSymbologyIsOneOfTheEngines() {
        val ed = editor()
        onMain { ed.addBarcode().join() }
        val samples = mapOf(
            "ean13" to "5901234123457", "ean8" to "96385074", "upc_a" to "036000291452", "upc_e" to "01234565",
            "itf" to "12345678", "gs1_128" to "(01)09501101530003", "code128_c" to "123456", "codabar" to "A1234B",
        )
        for ((id, _) in SYMBOLOGIES_1D) {
            assertNull(id, onMain { ed.applyCode(ed.selected!!, id, samples[id] ?: "ABC-123") }.error)
            assertEquals(id, ed.selected!!.symbology)
        }
    }

    // ---- images ----

    @Test
    fun anImageIsPlacedWithItsAspectRatio() {
        val ed = editor()
        val png = ByteArrayOutputStream().also { ImageIO.write(BufferedImage(200, 100, BufferedImage.TYPE_INT_RGB), "png", it) }.toByteArray()
        onMain { ed.addImage(png, "image/png", 200, 100).join() }
        val e = ed.selected!!
        assertEquals("image", e.type)
        assertEquals(Frame(1.0, 3.0, 48.0, 24.0), e.frame)
        onMain { ed.setProps(e.id, "dither" to "threshold", "threshold" to 100, "invert" to true).join() }
        val changed = ed.selected!!
        assertEquals("threshold", changed.dither)
        assertEquals(100, changed.threshold)
        assertTrue(changed.invert)
        for (d in DITHERS) onMain { ed.setProps(e.id, "dither" to d).join() }
        assertEquals(DITHERS.last(), ed.selected!!.dither)
    }

    // ---- printing and printer settings on the simulator ----

    @Test
    fun printsTheCurrentRecordOrAll() {
        val ed = editor()
        onMain {
            app.connectNow(SIM_ADDRESS)
            ed.attachDataNow("products.csv", csv.toByteArray())
        }
        ed.print(1, null, all = true)
        onMain { while (ed.progress?.finished != true) delay(10) }
        assertEquals("done", ed.progress!!.state)
        assertEquals(3, ed.progress!!.done)

        ed.print(2, null, all = false)
        onMain { while (ed.progress?.finished != true) delay(10) }
        assertEquals("done", ed.progress!!.state)
        assertEquals(2, ed.progress!!.done)
    }

    @Test
    fun printerSettingsAreReadAndWritten() {
        onMain {
            app.connectNow(SIM_ADDRESS)
            app.loadSettings()
        }
        val s = app.printerSettings!!
        assertNotNull(s.autoOffLevel)
        assertNotNull(s.soundPower)
        assertNotNull(s.soundBluetooth)

        val level = if (s.autoOffLevel == 4) 1 else 4
        onMain { app.setSettingsNow("auto_off_level" to level, "sound_power" to !s.soundPower!!) }
        assertEquals(s.copy(autoOffLevel = level, soundPower = !s.soundPower!!), app.printerSettings)
        onMain { app.setSettings("sound_bluetooth" to !s.soundBluetooth!!).join() }
        // Read back from the printer, not only remembered.
        onMain { app.loadSettings() }
        assertEquals(s.copy(autoOffLevel = level, soundPower = !s.soundPower!!, soundBluetooth = !s.soundBluetooth!!), app.printerSettings)
    }

    // ---- helpers ----

    /** A new 50 × 30 mm label, open in the editor. */
    private fun editor(): EditorState {
        onMain { app.newLabel(50.0, 30.0).join() }
        return app.editor!!
    }

    /** A minimal workbook with one sheet of inline strings. */
    private fun xlsx(rows: List<List<String>>): ByteArray {
        val sheet = rows.joinToString("") { r -> "<row>" + r.joinToString("") { """<c t="inlineStr"><is><t>$it</t></is></c>""" } + "</row>" }
        val parts = mapOf(
            "[Content_Types].xml" to """<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>""",
            "_rels/.rels" to """<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>""",
            "xl/workbook.xml" to """<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>""",
            "xl/_rels/workbook.xml.rels" to """<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>""",
            "xl/worksheets/sheet1.xml" to """<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>$sheet</sheetData></worksheet>""",
        )
        val out = ByteArrayOutputStream()
        ZipOutputStream(out).use { zip ->
            for ((name, xml) in parts) {
                zip.putNextEntry(ZipEntry(name))
                zip.write("""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>$xml""".toByteArray())
                zip.closeEntry()
            }
        }
        return out.toByteArray()
    }

    /** Run [block] on the main thread, where the app's state lives, and run the main looper until it is done. */
    @OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
    private fun <T> onMain(block: suspend () -> T): T {
        val job = GlobalScope.async(Dispatchers.Main) { block() }
        val looper = shadowOf(Looper.getMainLooper())
        val deadline = System.nanoTime() + 60_000_000_000L
        while (!job.isCompleted) {
            if (System.nanoTime() > deadline) throw AssertionError("call did not finish in 60 s")
            looper.idleFor(Duration.ofMillis(10))
            Thread.sleep(1)
        }
        job.getCompletionExceptionOrNull()?.let { throw it }
        return job.getCompleted()
    }
}
