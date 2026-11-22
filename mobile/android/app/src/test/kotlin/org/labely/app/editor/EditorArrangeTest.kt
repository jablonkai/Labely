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
 * Selecting, arranging, the clipboard, inserting and the paper library on the real engine (built for the host, loaded
 * through JNA) and the B21 Pro simulator, as in `LabelyFunctionsTest`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class EditorArrangeTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val app get() = (context as LabelyApplication).state

    private val csv = "Name;Price;EAN\nApple;649;5998001234561\nPear;899;5998001234562\nBanana;749;5998001234563\n"

    private fun EditorState.el(id: Long) = doc!!.elements.first { it.id == id }

    /** Two rectangles at known places; returns their ids. */
    private fun two(ed: EditorState): List<Long> {
        onMain { ed.addRect().join() }
        val a = ed.selectedId!!
        onMain { ed.exec("element.move", "id" to a, "x_mm" to 2.0, "y_mm" to 3.0).join() }
        onMain { ed.addRect().join() }
        val b = ed.selectedId!!
        onMain { ed.exec("element.move", "id" to b, "x_mm" to 25.0, "y_mm" to 12.0).join() }
        return listOf(a, b)
    }

    @Test
    fun severalElementsMoveInOneStep() {
        val ed = editor()
        val (a, b) = two(ed)
        onMain { ed.selectAll() }
        assertEquals(listOf(a, b), ed.selection)
        assertNull(ed.selectedId)
        assertEquals(2, ed.selectedElements.size)
        onMain { ed.moveBy(1.5, -1.0).join() }
        assertEquals(3.5, ed.el(a).xMm, 1e-6)
        assertEquals(2.0, ed.el(a).yMm, 1e-6)
        assertEquals(26.5, ed.el(b).xMm, 1e-6)
        // One undo takes both back.
        onMain { ed.undo().join() }
        assertEquals(2.0, ed.el(a).xMm, 1e-6)
        assertEquals(25.0, ed.el(b).xMm, 1e-6)
        // A locked element stays.
        onMain { ed.lock(listOf(a), true).join() }
        assertTrue(ed.el(a).locked)
        onMain { ed.moveBy(1.0, 0.0).join() }
        assertEquals(2.0, ed.el(a).xMm, 1e-6)
        assertEquals(26.0, ed.el(b).xMm, 1e-6)
        onMain { ed.toggle(a) }
        assertEquals(listOf(b), ed.selection)
        assertEquals(b, ed.selectedId)
    }

    @Test
    fun alignGroupHideAndOrder() {
        val ed = editor()
        val (a, b) = two(ed)
        onMain { ed.select(listOf(a, b)) }
        onMain { ed.align("left").join() }
        assertEquals(ed.el(a).xMm, ed.el(b).xMm, 1e-6)
        onMain { ed.group().join() }
        assertNotNull(ed.el(a).group)
        assertEquals(ed.el(a).group, ed.el(b).group)
        // Selecting one member takes the whole group.
        onMain { ed.select(listOf(a)) }
        assertEquals(listOf(a, b), ed.selection)
        onMain { ed.ungroup().join() }
        assertNull(ed.el(a).group)
        onMain { ed.setHidden(listOf(a, b), true).join() }
        assertTrue(ed.el(a).hidden && ed.el(b).hidden)
        onMain { ed.setHidden(listOf(a, b), false).join() }
        onMain { ed.reorder("front", listOf(a)).join() }
        assertEquals(listOf(b, a), ed.doc!!.elements.map { it.id })
        onMain { ed.reorder("back", listOf(a)).join() }
        assertEquals(listOf(a, b), ed.doc!!.elements.map { it.id })
    }

    @Test
    fun rotateFlipAndMirror() {
        val ed = editor()
        val (a, _) = two(ed)
        onMain { ed.rotate(a, 370.0).join() }
        assertEquals(10.0, ed.el(a).rotationDeg, 1e-6)
        assertEquals(8, Handle.of(ed.el(a)).size)
        onMain { ed.select(listOf(a)) }
        onMain { ed.flip(true).join() }
        assertTrue(ed.el(a).flipH)
        assertFalse(ed.el(a).flipV)
        onMain { ed.mirror(a, "horizontal").join() }
        val copy = ed.selected!!
        assertEquals(a, copy.mirrorOf)
        assertEquals(3, ed.doc!!.elements.size)
    }

    @Test
    fun copyCutAndPaste() {
        val ed = editor()
        val (a, b) = two(ed)
        onMain { ed.select(listOf(a, b)) }
        onMain { ed.copy().join() }
        assertNotNull(app.clip)
        onMain { ed.paste()!!.join() }
        assertEquals(4, ed.doc!!.elements.size)
        // The copies are selected and set off by 2 mm.
        assertEquals(2, ed.selection.size)
        assertEquals(4.0, ed.selectedElements.minOf { it.xMm }, 1e-6)
        onMain { ed.cut().join() }
        assertEquals(2, ed.doc!!.elements.size)
        // Into another document they go where they were.
        val other = editor()
        assertNotSame(ed.docId, other.docId)
        onMain { other.paste()!!.join() }
        assertEquals(2, other.doc!!.elements.size)
        assertEquals(4.0, other.doc!!.elements.minOf { it.xMm }, 1e-6)
        // Plain text copied elsewhere becomes a text element.
        onMain { app.setSystemClipboard("Hello from outside") }
        onMain { other.pasteAny()!!.join() }
        assertEquals("text", other.selected!!.type)
        assertEquals("Hello from outside", other.selected!!.text)
    }

    @Test
    fun insertsTableBorderAndBoundTexts() {
        val ed = editor()
        onMain { ed.addTable(2, 3).join() }
        val t = ed.selected!!
        assertEquals("table", t.type)
        assertEquals(2, t.raw.getJSONArray("row_heights").length())
        assertEquals(3, t.raw.getJSONArray("column_widths").length())
        onMain { ed.addBorder("double").join() }
        assertEquals("border", ed.selected!!.type)
        assertEquals("double", ed.selected!!.raw.optString("style"))
        onMain { ed.addBound(EditorState.DATE_BINDING).join() }
        assertEquals("text", ed.selected!!.type)
        assertTrue(ed.selected!!.text!!.contains("{{"))
        onMain { ed.setProps(ed.selectedId!!, "name" to "Best before").join() }
        assertEquals("Best before", ed.selected!!.raw.optString("name"))
        onMain { ed.setSpans(ed.selectedId!!, listOf(Span(0, 2, bold = true))).join() }
        assertEquals(listOf(Span(0, 2, bold = true)), spansOf(ed.selected!!))
    }

    @Test
    fun serialNumbersAreKeptInTheDocument() {
        val ed = editor()
        onMain { ed.setSerial("batch", 7, 1, 4, "B", "").join() }
        val s = ed.doc!!.serials.single()
        assertEquals("batch", s.name)
        assertEquals("B0007", s.value(0))
        onMain { ed.removeSerial("batch").join() }
        assertTrue(ed.doc!!.serials.isEmpty())
    }

    @Test
    fun paperLibraryAppliesAPaper() {
        val ed = editor()
        val papers = onMain { org.labely.app.engine.Paper.list(app.engine.call("paper.list")) }
        assertTrue(papers.isNotEmpty())
        val p = papers.first { it.widthMm != 50.0 || it.heightMm != 30.0 }
        onMain { ed.applyPaper(p.id).join() }
        assertEquals(p.widthMm, ed.doc!!.label.widthMm, 1e-6)
        assertEquals(p.heightMm, ed.doc!!.label.heightMm, 1e-6)
        onMain { ed.setLabelProps("corner_radius_mm" to 2.5, "name" to "Shelf").join() }
        assertEquals(2.5, ed.doc!!.label.cornerRadiusMm, 1e-6)
        assertEquals("Shelf", ed.doc!!.name)
        onMain { app.loadModels() }
        assertTrue(app.modelIds.isNotEmpty())
    }

    @Test
    fun tableCellsRowsAndColumnsAreEdited() {
        val ed = editor()
        onMain { ed.addTable(2, 2).join() }
        val id = ed.selectedId!!
        onMain { ed.tableEdit(id, "insert_row", "at" to 1).join() }
        onMain { ed.tableEdit(id, "insert_column", "at" to 0).join() }
        onMain { ed.tableEdit(id, "set_cell", "row" to 0, "col" to 0, "text" to "Kg").join() }
        val t = ed.el(id)
        assertEquals(3, t.raw.getJSONArray("row_heights").length())
        assertEquals(3, t.raw.getJSONArray("column_widths").length())
        assertTrue(t.raw.toString().contains("Kg"))
        onMain { ed.tableEdit(id, "delete_row", "at" to 2).join() }
        assertEquals(2, ed.el(id).raw.getJSONArray("row_heights").length())
    }

    @Test
    fun printsARangeWithOptionsAndKeepsHistory() {
        val ed = editor()
        onMain {
            app.connectNow(SIM_ADDRESS)
            ed.attachDataNow("products.csv", csv.toByteArray())
            app.clearHistory().join()
        }
        assertTrue(app.history.isEmpty())
        val picked = parseRecords("1,3", ed.recordCount)!!
        ed.print(PrintOptions(copies = 2, records = recordsParam("1, 3"), offsetXMm = 0.5, offsetYMm = -0.5, mirror = true, rotate180 = true, total = picked.size * 2))
        onMain { while (ed.progress?.finished != true) delay(10) }
        assertEquals("done", ed.progress!!.state)
        assertEquals(4, ed.progress!!.done)
        // Serial numbers: a count of labels without data rows.
        onMain { ed.detachData().join() }
        ed.print(PrintOptions(count = 3, total = 3))
        onMain { while (ed.progress?.finished != true) delay(10) }
        assertEquals("done", ed.progress!!.state)
        assertEquals(3, ed.progress!!.done)
        // The simulated B21 Pro prints gray levels.
        assertTrue(app.printer?.model?.gray == true)
        ed.print(PrintOptions(grayscale = true, total = 1))
        onMain { while (ed.progress?.finished != true) delay(10) }
        assertEquals("done", ed.progress!!.state)

        onMain { app.refreshHistory() }
        assertEquals(3, app.history.size)
        assertTrue(app.history.all { it.state == "done" })
        onMain { app.refreshJobs() }
        assertTrue(app.jobs.all { it.finished })
        onMain { app.pauseQueue(true).join() }
        assertTrue(app.queuePaused)
        onMain { app.pauseQueue(false).join() }
        assertFalse(app.queuePaused)
        onMain { app.clearJobs().join() }
        assertTrue(app.jobs.isEmpty())
        onMain { app.deleteHistory(app.history.first()).join() }
        assertEquals(2, app.history.size)
    }

    private fun editor(): EditorState {
        onMain { app.newLabel(50.0, 30.0).join() }
        return app.editor!!
    }

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
