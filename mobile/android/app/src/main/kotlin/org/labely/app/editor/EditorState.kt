package org.labely.app.editor

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import java.io.File
import java.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.labely.app.AppState
import org.labely.app.Recent
import org.labely.app.engine.CommandFailure
import org.labely.app.engine.DataPage
import org.labely.app.engine.Doc
import org.labely.app.engine.Element
import org.labely.app.engine.Issue
import org.labely.app.engine.json

/** Print progress of the editor's document. */
data class PrintProgress(val done: Int, val total: Int, val state: String, val error: String?) {
    val finished: Boolean get() = state == "done" || state == "failed" || state == "cancelled"
}

/** The attached data source: where it came from, its columns and its number of records. */
data class DataInfo(val origin: String?, val columns: List<String>, val rowCount: Int)

/** What the engine said about a barcode value: [error] when it cannot be encoded. */
data class CodeCheck(val error: String?, val checkDigit: Int? = null)

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

    /** The data source, the rows read so far (paged) and the record the canvas shows (1-based). */
    var data by mutableStateOf<DataInfo?>(null)
        private set
    var rows by mutableStateOf<List<List<String>>>(emptyList())
        private set
    var record by mutableIntStateOf(1)
        private set
    private var loadingRows = false

    /** Name of the picked CSV file; the engine keeps no origin for CSV text. */
    private var pickedName: String? = null

    /** The barcode or 2D code whose sheet is open. */
    var codeSheet by mutableStateOf<Long?>(null)

    val selected: Element? get() = doc?.elements?.firstOrNull { it.id == selectedId }

    /** Number of records, 0 without data. */
    val recordCount: Int get() = data?.rowCount ?: 0

    val sourceName: String? get() = data?.origin ?: pickedName

    /** Elements that show a column of the data source. */
    val boundCount: Int get() = data?.let { d -> doc?.elements?.count { boundColumn(it.content, d.columns) != null } } ?: 0

    fun boundColumn(e: Element): String? = data?.let { boundColumn(e.content, it.columns) }

    suspend fun refresh() {
        val d = Doc.from(engine.call("doc.get", "doc_id" to docId))
        val page = DataPage.from(engine.call("data.get", "doc_id" to docId, "from" to 1, "limit" to rows.size.coerceIn(PAGE, MAX_PAGE)))
        data = if (page.attached) DataInfo(page.origin, page.columns, page.rowCount) else null
        rows = page.rows
        record = record.coerceIn(1, page.rowCount.coerceAtLeast(1))
        preview = render()
        doc = d
        if (d.elements.none { it.id == selectedId }) selectedId = null
    }

    /** The print raster; with data, of the previewed record. */
    private suspend fun render(): ImageBitmap =
        if (recordCount > 0) engine.preview(PREVIEW_DPI, docId, record) else engine.render(PREVIEW_DPI, docId)

    /** Run a document command; with [select], the element it creates becomes the selection. */
    fun exec(id: String, params: JSONObject = JSONObject(), select: Boolean = false) = app.scope.launch {
        app.report {
            val r = engine.call(id, params.put("doc_id", docId))
            if (select && r.has("id")) selectedId = r.getLong("id")
            changed()
        }
    }

    /** After a change: read the document back and save it. */
    private suspend fun changed() {
        saved = false
        refresh()
        save()
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

    fun addBarcode() = addCode("element.add_barcode", frame(36.0, 12.0).put("data", "123456789012"))

    fun addQr() = addCode("element.add_code2d", frame(16.0, 16.0).put("data", "https://labely.org"))

    /** Add a code and open its sheet. */
    private fun addCode(id: String, params: JSONObject) = app.scope.launch {
        exec(id, params, select = true).join()
        codeSheet = selected?.takeIf { it.type == "barcode" || it.type == "code2d" }?.id
    }

    /**
     * Add a picture ([bytes] of type [mime], [widthPx] × [heightPx]) in a frame of its aspect
     * ratio inside the label.
     */
    fun addImage(bytes: ByteArray, mime: String, widthPx: Int, heightPx: Int): Job {
        val l = doc?.label
        val f = if (l != null) imageFrame(l.widthMm, l.heightMm, widthPx, heightPx) else Frame(0.0, 0.0, 20.0, 20.0)
        return exec(
            "element.add_image",
            json("x_mm" to f.xMm, "y_mm" to f.yMm, "width_mm" to f.widthMm, "height_mm" to f.heightMm, "data_base64" to Base64.getEncoder().encodeToString(bytes), "mime" to mime),
            select = true,
        )
    }

    /** Move and size an element in one command, so a handle drag is one undo step. */
    fun resize(id: Long, f: Frame) = setProps(id, "x_mm" to f.xMm, "y_mm" to f.yMm, "width_mm" to f.widthMm, "height_mm" to f.heightMm)

    // ---- data ----

    /** Attach a picked file: a spreadsheet through a copy in the engine's data dir, else as CSV text. */
    fun attachData(name: String, bytes: ByteArray) = app.scope.launch { app.report { attachDataNow(name, bytes) } }

    suspend fun attachDataNow(name: String, bytes: ByteArray) {
        if (isSpreadsheet(name, bytes)) {
            // The engine reads files only inside its data dir; the rows are embedded, so the copy can go.
            val file = File(app.importsDir, exportBaseName(name))
            try {
                withContext(Dispatchers.IO) {
                    app.importsDir.mkdirs()
                    file.writeBytes(bytes)
                }
                engine.call("data.attach_xlsx", "doc_id" to docId, "path" to file.path)
            } finally {
                withContext(Dispatchers.IO) { file.delete() }
            }
            pickedName = null
        } else {
            engine.call("data.attach_csv", "doc_id" to docId, "text" to decodeText(bytes))
            pickedName = name
        }
        record = 1
        rows = emptyList()
        changed()
    }

    fun detachData(): Job {
        pickedName = null
        return exec("data.detach")
    }

    /** Show record [n] (1-based, clamped) on the canvas. */
    fun showRecord(n: Int) = app.scope.launch {
        app.report {
            val r = n.coerceIn(1, recordCount.coerceAtLeast(1))
            if (r != record) {
                record = r
                preview = render()
            }
        }
    }

    /** Read the next page of rows. */
    fun loadMoreRows() = app.scope.launch {
        app.report {
            if (loadingRows || rows.size >= recordCount) return@report
            loadingRows = true
            try {
                val page = DataPage.from(engine.call("data.get", "doc_id" to docId, "from" to rows.size + 1, "limit" to PAGE))
                if (page.from == rows.size + 1) rows = rows + page.rows
            } finally {
                loadingRows = false
            }
        }
    }

    /** The previewed record's value of [column], when its row is loaded. */
    fun value(column: String): String? {
        val i = data?.columns?.indexOf(column) ?: return null
        return rows.getOrNull(record - 1)?.getOrNull(i)
    }

    fun bind(id: Long, column: String) = exec("data.bind", "id" to id, "column" to column)

    /**
     * Remove an element's binding. A binding is `{{column}}` in the element's string, so the
     * string is set back to plain text: the previewed record's value.
     */
    fun unbind(e: Element): Job? {
        val column = boundColumn(e) ?: return null
        val content = e.content ?: return null
        val plain = unbound(content, column, value(column)?.ifEmpty { null } ?: column)
        return setProps(e.id, (if (e.type == "text") "text" else "data") to plain)
    }

    // ---- barcodes and 2D codes ----

    /**
     * Set a code's [symbology] and [data] if the engine can encode them; otherwise the element
     * keeps its value and the engine's message is returned. Bound data is not checked: its
     * values come from the records.
     */
    suspend fun applyCode(e: Element, symbology: String, data: String): CodeCheck =
        try {
            val error = if ("{{" in data) null else encodeError(e, symbology, data)
            if (error != null) {
                CodeCheck(error)
            } else {
                val props = json("symbology" to symbology.takeIf { it != e.symbology }, "data" to data.takeIf { it != e.data })
                if (props.length() > 0) {
                    engine.call("element.set_props", "doc_id" to docId, "id" to e.id, "props" to props)
                    changed()
                }
                CodeCheck(null, gtinCheckDigit(symbology, data))
            }
        } catch (f: CommandFailure) {
            CodeCheck(f.message?.ifEmpty { f.code } ?: f.code)
        }

    /**
     * `element.set_props` takes any string as code data; whether it encodes is a finding of
     * `doc.check` (`encode_failed`). So the value is tried on a scratch document first, which
     * keeps a rejected value out of this document and its undo history.
     */
    private suspend fun encodeError(e: Element, symbology: String, data: String): String? {
        val l = doc?.label ?: return null
        val scratch = engine.call("doc.new", "width_mm" to l.widthMm, "height_mm" to l.heightMm).getString("doc_id")
        try {
            engine.call(
                if (e.type == "barcode") "element.add_barcode" else "element.add_code2d",
                "doc_id" to scratch, "x_mm" to e.xMm, "y_mm" to e.yMm, "width_mm" to e.widthMm, "height_mm" to e.heightMm,
                "data" to data, "symbology" to symbology,
            )
            val issue = Issue.list(engine.call("doc.check", "doc_id" to scratch)).firstOrNull { it.rule == "encode_failed" }
            return issue?.message?.let { if (it.startsWith("element ")) it.substringAfter(": ") else it }
        } finally {
            engine.call("doc.close", "doc_id" to scratch)
        }
    }

    // ---- export ----

    /**
     * Write the label into the engine's `exports/` folder (emptied first) and return the files:
     * one PNG or PDF, or with [all] records one PNG per record (a PDF holds them as pages).
     */
    suspend fun export(format: ExportFormat, dpi: Int, all: Boolean): List<File> {
        val d = doc ?: return emptyList()
        val dir = app.exportsDir
        withContext(Dispatchers.IO) {
            dir.deleteRecursively()
            dir.mkdirs()
        }
        val records = if (recordCount > 0) (if (all) (1..recordCount).toList() else listOf(record)) else null
        val files = exportFileNames(d.name, format, records?.takeIf { all }).map { File(dir, it) }
        when (format) {
            ExportFormat.Pdf -> engine.call(
                "export.pdf",
                "doc_id" to docId, "path" to files[0].path, "dpi" to dpi, "records" to records?.let { if (all) "all" else record.toString() },
            )
            ExportFormat.Png -> files.forEachIndexed { i, f ->
                engine.call("export.png", "doc_id" to docId, "path" to f.path, "dpi" to dpi, "record" to records?.get(i))
            }
        }
        return files
    }

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

    /** Print [copies] of the previewed record, or with [all] of every record. */
    fun print(copies: Int, density: Int?, all: Boolean = false) {
        val printer = app.printer ?: return
        printJob?.cancel()
        val records = if (recordCount > 0) (if (all) "all" else record.toString()) else null
        val total = copies * if (all) recordCount.coerceAtLeast(1) else 1
        progress = PrintProgress(0, total, "queued", null)
        printJob = app.scope.launch {
            app.report {
                try {
                    val r = engine.call(
                        "print.submit",
                        "doc_id" to docId, "address" to printer.address, "copies" to copies, "density" to density, "records" to records, "wait" to false,
                    )
                    val jobId = r.getString("job_id")
                    // The engine pushes each change (ADR-0031); stop at the final state.
                    engine.job(jobId).first { j ->
                        val p = PrintProgress(
                            done = j.optInt("pages_done"),
                            total = j.optInt("pages_total", total),
                            state = j.optString("state"),
                            error = j.optJSONObject("error")?.optString("message"),
                        )
                        progress = p
                        p.finished
                    }
                } catch (e: Exception) {
                    progress = PrintProgress(0, total, "failed", e.message)
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

        /** Rows read per `data.get`; the engine gives at most [MAX_PAGE]. */
        const val PAGE = 50
        const val MAX_PAGE = 200
    }
}
