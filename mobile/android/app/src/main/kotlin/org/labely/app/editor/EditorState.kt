package org.labely.app.editor

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import org.json.JSONObject
import org.labely.app.AppState
import org.labely.app.Recent
import org.labely.app.engine.Doc
import org.labely.app.engine.Element
import org.labely.app.engine.Issue
import org.labely.app.engine.json

/** Print progress of the editor's document. */
data class PrintProgress(val done: Int, val total: Int, val state: String, val error: String?) {
    val finished: Boolean get() = state == "done" || state == "failed" || state == "cancelled"
}

/**
 * One open document. Every change is an engine command with this document's `doc_id`; after it
 * the document and its print raster are read back, so what the canvas shows is what prints.
 */
class EditorState(val app: AppState, val docId: String) {
    private val engine = app.engine

    var doc by mutableStateOf<Doc?>(null)
        private set
    var preview by mutableStateOf<ImageBitmap?>(null)
        private set
    var selectedId by mutableStateOf<Long?>(null)
    var issues by mutableStateOf<List<Issue>?>(null)
        private set
    var progress by mutableStateOf<PrintProgress?>(null)
        private set

    /** False while an edit is not yet on disk. Edits are saved as they happen. */
    var saved by mutableStateOf(true)
        private set
    private var printJob: Job? = null

    val selected: Element? get() = doc?.elements?.firstOrNull { it.id == selectedId }

    suspend fun refresh() {
        val d = Doc.from(engine.call("doc.get", "doc_id" to docId))
        preview = engine.render(PREVIEW_DPI, docId)
        doc = d
        if (d.elements.none { it.id == selectedId }) selectedId = null
    }

    /** Run a document command; with [select], the element it creates becomes the selection. */
    fun exec(id: String, params: JSONObject = JSONObject(), select: Boolean = false) = app.scope.launch {
        app.report {
            val r = engine.call(id, params.put("doc_id", docId))
            if (select && r.has("id")) selectedId = r.getLong("id")
            saved = false
            refresh()
            save()
        }
    }

    fun exec(id: String, vararg params: Pair<String, Any?>, select: Boolean = false) = exec(id, json(*params), select)

    fun setProps(id: Long, vararg props: Pair<String, Any?>) =
        exec("element.set_props", "id" to id, "props" to json(*props))

    fun undo() = exec("history.undo")

    fun redo() = exec("history.redo")

    // ---- adding elements ----

    /** A frame centred on the label, at most [w] × [h] mm. */
    private fun frame(w: Double, h: Double): JSONObject {
        val l = doc?.label ?: return json("x_mm" to 0, "y_mm" to 0, "width_mm" to w, "height_mm" to h)
        val fw = minOf(w, l.widthMm - 2)
        val fh = minOf(h, l.heightMm - 2)
        return json("x_mm" to (l.widthMm - fw) / 2, "y_mm" to (l.heightMm - fh) / 2, "width_mm" to fw, "height_mm" to fh)
    }

    fun addText(text: String) = exec("element.add_text", frame(40.0, 8.0).put("text", text).put("fit", "shrink"), select = true)

    fun addBarcode() = exec("element.add_barcode", frame(36.0, 12.0).put("data", "123456789012"), select = true)

    fun addQr() = exec("element.add_code2d", frame(16.0, 16.0).put("data", "https://labely.org"), select = true)

    fun addRect() = exec("element.add_rect", frame(20.0, 12.0).put("stroke_mm", 0.4), select = true)

    fun addEllipse() = exec("element.add_ellipse", frame(16.0, 12.0).put("stroke_mm", 0.4), select = true)

    fun addLine() = exec("element.add_line", frame(30.0, 0.0).put("height_mm", 0.0).put("stroke_mm", 0.4), select = true)

    // ---- label ----

    fun setLabel(widthMm: Double, heightMm: Double, media: String) =
        exec("doc.set_label", "width_mm" to widthMm, "height_mm" to heightMm, "media" to media)

    fun saveTemplate(name: String) = app.scope.launch {
        app.report {
            engine.call("template.save", "doc_id" to docId, "name" to name, "overwrite" to true)
            app.loadTemplates()
        }
    }

    /** Save to the document's own path, or a new file in app storage. */
    suspend fun save() {
        val d = doc ?: return
        val path = d.path ?: app.newDocPath()
        engine.call("doc.save", "doc_id" to docId, "path" to path)
        if (d.path == null) doc = d.copy(path = path)
        saved = true
        app.remember(Recent(path, d.name, d.label.widthMm, d.label.heightMm), preview)
    }

    // ---- printing ----

    fun check() = app.scope.launch {
        issues = null
        app.report {
            issues = Issue.list(engine.call("doc.check", "doc_id" to docId, "address" to app.printer?.address))
        }
    }

    fun print(copies: Int, density: Int?) {
        val printer = app.printer ?: return
        printJob?.cancel()
        progress = PrintProgress(0, copies, "queued", null)
        printJob = app.scope.launch {
            app.report {
                try {
                    val r = engine.call(
                        "print.submit",
                        "doc_id" to docId, "address" to printer.address, "copies" to copies, "density" to density, "wait" to false,
                    )
                    val jobId = r.getString("job_id")
                    // The engine pushes each change (ADR-0031); stop at the final state.
                    engine.job(jobId).first { j ->
                        val p = PrintProgress(
                            done = j.optInt("pages_done"),
                            total = j.optInt("pages_total", copies),
                            state = j.optString("state"),
                            error = j.optJSONObject("error")?.optString("message"),
                        )
                        progress = p
                        p.finished
                    }
                } catch (e: Exception) {
                    progress = PrintProgress(0, copies, "failed", e.message)
                    throw e
                }
                app.refreshHistory()
                app.refreshStatus()
            }
        }
    }

    fun clearProgress() {
        if (progress?.finished == true) progress = null
    }

    companion object {
        /** The B21 Pro's own resolution, so the canvas shows the exact raster. */
        const val PREVIEW_DPI = 203
    }
}
