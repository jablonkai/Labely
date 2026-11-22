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
import org.json.JSONArray
import org.json.JSONObject
import org.labely.app.AppState
import org.labely.app.Recent
import org.labely.app.engine.CommandFailure
import org.labely.app.engine.DataPage
import org.labely.app.engine.Doc
import org.labely.app.engine.Element
import org.labely.app.engine.Issue
import org.labely.app.engine.json
import org.labely.app.engine.objects

/** Print progress of the editor's document. */
data class PrintProgress(val done: Int, val total: Int, val state: String, val error: String?) {
    val finished: Boolean get() = state == "done" || state == "failed" || state == "cancelled"
}

/** How a job is printed, beyond the document: `print.submit`'s options. */
data class PrintOptions(
    val copies: Int = 1,
    /** Null prints with the label's density, else the model's default. */
    val density: Int? = null,
    /** Records as the engine reads them (`all`, `3`, `1,4,6-9`); null without data. */
    val records: String? = null,
    /** Labels to print without data (serials advance per label); null is one. */
    val count: Int? = null,
    /** Column holding each record's copy count; overrides [copies]. */
    val copiesColumn: String? = null,
    val offsetXMm: Double = 0.0,
    val offsetYMm: Double = 0.0,
    val mirror: Boolean = false,
    val rotate180: Boolean = false,
    /** 16 gray levels; only for a model with [org.labely.app.engine.Model.gray]. */
    val grayscale: Boolean = false,
    /** Labels expected, for the progress bar until the engine reports its own total. */
    val total: Int = 1,
)

/** A barcode or 2D code `code.decode` found in a picture. */
data class FoundCode(val kind: String, val symbology: String, val text: String)

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
    /** The selected elements, in z-order. A group is selected as a whole. */
    var selection by mutableStateOf<List<Long>>(emptyList())
        private set

    /** While on, a tap adds to or takes from the selection and a drag on empty paper draws a marquee. */
    var multiSelect by mutableStateOf(false)

    /** The only selected element, for the inspector; null for none or several. */
    var selectedId: Long?
        get() = selection.singleOrNull()
        set(id) = select(listOfNotNull(id))
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

    /** The table whose cell editor is open. */
    var tableSheet by mutableStateOf<Long?>(null)

    val selected: Element? get() = selectedId?.let { id -> doc?.elements?.firstOrNull { it.id == id } }

    val selectedElements: List<Element> get() = doc?.elements.orEmpty().filter { it.id in selection }

    /** The picked spreadsheet's sheets, the one read and whether its first row names the columns. */
    var sheets by mutableStateOf<List<String>>(emptyList())
        private set
    var sheet by mutableStateOf<String?>(null)
        private set
    var header by mutableStateOf(true)
        private set
    private var pickedBytes: ByteArray? = null

    /** The running print job, for Cancel. */
    private var jobId: String? = null

    /** Select [ids] with their groups; leaving the last one ends multi-select. */
    fun select(ids: Collection<Long>) {
        selection = withGroups(ids, doc?.elements.orEmpty())
        if (selection.isEmpty()) multiSelect = false
    }

    /** Add [id] to the selection or take it out (multi-select). */
    fun toggle(id: Long) {
        selection = toggled(selection, id, doc?.elements.orEmpty())
    }

    fun selectAll() {
        selection = doc?.elements.orEmpty().filter { !it.hidden }.map { it.id }
        multiSelect = selection.size > 1
    }

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
        val left = selection.filter { id -> d.elements.any { it.id == id } }
        if (left != selection) select(left)
    }

    /** The print raster; with data, of the previewed record. */
    private suspend fun render(): ImageBitmap =
        if (recordCount > 0) engine.preview(PREVIEW_DPI, docId, record) else engine.render(PREVIEW_DPI, docId)

    /** Run a document command; with [select], the element it creates becomes the selection. */
    fun exec(id: String, params: JSONObject = JSONObject(), select: Boolean = false) = app.scope.launch {
        app.report {
            val r = engine.call(id, params.put("doc_id", docId))
            val made = if (!select) null else if (r.has("id")) listOf(r.getLong("id")) else r.optJSONArray("ids")?.let { a -> (0 until a.length()).map { a.getLong(it) } }
            changed()
            if (made != null) {
                multiSelect = made.size > 1
                select(made)
            }
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

    /** Several commands as one undo step. */
    fun batch(steps: List<Pair<String, JSONObject>>) = app.scope.launch {
        if (steps.isEmpty()) return@launch
        app.report {
            engine.batch(docId, steps)
            changed()
        }
    }

    // ---- arranging the selection ----

    private fun ids(list: List<Long> = selection) = JSONArray(list)

    fun deleteSelection() = exec("element.delete", "ids" to ids())

    /** Copies of the selection, 1 mm down and right; groups stay groups. */
    fun duplicateSelection(): Job {
        val one = selected
        if (one != null) return exec("element.duplicate", "id" to one.id, "dx_mm" to 1.0, "dy_mm" to 1.0, select = true)
        return exec("element.paste", json("elements" to JSONArray(selectedElements.map { it.raw }), "dx_mm" to 1.0, "dy_mm" to 1.0), select = true)
    }

    /** Align to the selection's box, or one element to the label. [edge]: left, center, right, top, middle, bottom. */
    fun align(edge: String) = exec("element.align", "ids" to ids(), "edge" to edge)

    /** Equal gaps between three or more elements. [axis]: horizontal or vertical. */
    fun distribute(axis: String) = exec("element.distribute", "ids" to ids(), "axis" to axis)

    fun group() = exec("element.group", "ids" to ids())

    fun ungroup() = exec("element.ungroup", "ids" to ids())

    fun lock(ids: List<Long>, locked: Boolean) = exec("element.lock", "ids" to ids(ids), "locked" to locked)

    fun setHidden(ids: List<Long>, hidden: Boolean) = batch(ids.map { "element.set_props" to json("id" to it, "props" to json("hidden" to hidden)) })

    /** A linked copy mirrored across the label's centre line. [axis]: horizontal or vertical. */
    fun mirror(id: Long, axis: String) = exec("element.mirror", "id" to id, "axis" to axis, select = true)

    /** Restack the selection, keeping its own order. [to]: front, back, forward, backward. */
    fun reorder(to: String, ids: List<Long> = selection): Job {
        val ordered = doc?.elements.orEmpty().map { it.id }.filter { it in ids }
        // Going up, the topmost moves first so the others do not jump over it.
        val steps = (if (to == "front" || to == "backward") ordered else ordered.reversed()).map { "element.reorder" to json("id" to it, "to" to to) }
        return batch(steps)
    }

    fun rotate(id: Long, deg: Double) = exec("element.rotate", "id" to id, "rotation_deg" to normDeg(deg))

    /** Mirror the selection's content inside its frames. */
    fun flip(horizontal: Boolean) = batch(
        selectedElements.filter { !it.locked }.map {
            "element.set_props" to json("id" to it.id, "props" to json((if (horizontal) "flip_h" else "flip_v") to !(if (horizontal) it.flipH else it.flipV)))
        },
    )

    /** Move the unlocked part of the selection by ([dxMm], [dyMm]) in one undo step. */
    fun moveBy(dxMm: Double, dyMm: Double, ids: List<Long> = selection): Job {
        val moving = doc?.elements.orEmpty().filter { it.id in ids && !it.locked }
        return batch(moving.map { "element.move" to json("id" to it.id, "x_mm" to round1(it.xMm + dxMm), "y_mm" to round1(it.yMm + dyMm)) })
    }

    // ---- clipboard ----

    /** Put the selection, with the pictures it uses, on the app's clipboard. */
    fun copy() = app.scope.launch {
        app.report {
            val picked = selectedElements
            if (picked.isEmpty()) return@report
            val assets = engine.call("doc.get", "doc_id" to docId).getJSONObject("document").optJSONObject("assets") ?: JSONObject()
            val used = JSONObject()
            for (e in picked) e.raw.optString("asset").let { key -> if (assets.has(key)) used.put(key, assets.get(key)) }
            // The texts also go to the system clipboard, which then tells which copy is newer.
            val text = picked.mapNotNull { it.content }.joinToString("\n")
            app.clip = json("elements" to JSONArray(picked.map { it.raw }), "assets" to used, "doc_id" to docId, "text" to text)
            if (text.isNotBlank()) app.setSystemClipboard(text)
        }
    }

    fun cut() = app.scope.launch {
        copy().join()
        if (app.clip != null) deleteSelection().join()
    }

    /** Paste the app's clipboard; in the document it came from the copies are set off by 2 mm. */
    fun paste(): Job? {
        val clip = app.clip ?: return null
        val d = if (clip.optString("doc_id") == docId) 2.0 else 0.0
        return exec("element.paste", json("elements" to clip.getJSONArray("elements"), "assets" to clip.optJSONObject("assets"), "dx_mm" to d, "dy_mm" to d), select = true)
    }

    /**
     * Paste what was copied last: the elements copied in the app, or plain text copied
     * elsewhere since, which becomes a text element.
     */
    fun pasteAny(): Job? {
        val system = app.systemClipboard()
        val clip = app.clip
        return if (system != null && (clip == null || system != clip.optString("text"))) addText(system) else paste()
    }

    /** Set a text's content; style ranges past the new end are cut off with it. */
    fun setText(e: Element, text: String): Job {
        val spans = clampSpans(spansOf(e), text.codePointCount(0, text.length))
        val props = json("text" to text)
        if (e.raw.has("spans")) props.put("spans", JSONArray(spans.map { json("start" to it.start, "end" to it.end, "bold" to it.bold, "italic" to it.italic, "underline" to it.underline, "font_size_pt" to it.fontSizePt) }))
        return setRaw(e.id, props)
    }

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

    fun addTable(rows: Int, columns: Int) = exec("element.add_table", frame(40.0, 20.0).put("row_count", rows).put("column_count", columns), select = true)

    fun addIcon(icon: String) = exec("element.add_icon", frame(12.0, 12.0).put("icon", icon), select = true)

    /** A decorative frame 1 mm inside the label's edge. */
    fun addBorder(style: String): Job {
        val l = doc?.label
        val f = if (l != null && l.widthMm > 4 && l.heightMm > 4) Frame(1.0, 1.0, l.widthMm - 2, l.heightMm - 2) else Frame(0.0, 0.0, 20.0, 12.0)
        return exec("element.add_border", json("x_mm" to f.xMm, "y_mm" to f.yMm, "width_mm" to f.widthMm, "height_mm" to f.heightMm, "style" to style), select = true)
    }

    /**
     * A text showing a computed value, as the studio's Insert menu does: [binding] is
     * `{{now:%Y-%m-%d}}`, `{{now:%H:%M}}` or `{{serial}}`. The serial counter is created when
     * the document has none.
     */
    fun addBound(binding: String) = app.scope.launch {
        if (binding == SERIAL_BINDING && doc?.serials?.none { it.name == "serial" } != false) {
            app.report { engine.call("data.set_serial", "doc_id" to docId) }
        }
        addText(binding).join()
    }

    /** Append a computed value ([DATE_BINDING], [TIME_BINDING], [SERIAL_BINDING]) to text [e]. */
    fun appendBinding(e: Element, binding: String) = app.scope.launch {
        if (binding == SERIAL_BINDING && doc?.serials?.none { it.name == "serial" } != false) {
            app.report { engine.call("data.set_serial", "doc_id" to docId) }
        }
        val text = e.text.orEmpty()
        setText(e, text + (if (text.isEmpty() || text.endsWith(' ')) "" else " ") + binding).join()
    }

    /** The barcodes and 2D codes in a picture (`code.decode`). */
    suspend fun decode(bytes: ByteArray): List<FoundCode> =
        engine.call("code.decode", "data_base64" to Base64.getEncoder().encodeToString(bytes)).optJSONArray("codes").objects()
            .map { FoundCode(it.optString("kind"), it.optString("symbology"), it.optString("text")) }

    /** An element that prints the code found in a picture again. */
    fun addCode(found: FoundCode): Job =
        if (found.kind == "barcode") {
            exec("element.add_barcode", frame(36.0, 12.0).put("data", found.text).put("symbology", found.symbology), select = true)
        } else {
            exec("element.add_code2d", frame(16.0, 16.0).put("data", found.text).put("symbology", found.symbology), select = true)
        }

    fun tableEdit(id: Long, op: String, vararg params: Pair<String, Any?>) =
        exec("element.table_edit", "id" to id, "edit" to json("op" to op, *params))

    /** Replace the rich-text ranges of text [id]. */
    fun setSpans(id: Long, spans: List<Span>) = exec(
        "element.set_props",
        "id" to id,
        "props" to json(
            "spans" to JSONArray(spans.map { json("start" to it.start, "end" to it.end, "bold" to it.bold, "italic" to it.italic, "underline" to it.underline, "font_size_pt" to it.fontSizePt) }),
        ),
    )

    /** Set properties where `null` means "back to the default" (e.g. `crop`, `dash_mm`, `name`). */
    fun setRaw(id: Long, props: JSONObject) = exec("element.set_props", "id" to id, "props" to props)

    // ---- serials and variables ----

    fun setSerial(name: String, start: Long, step: Long, digits: Int, prefix: String, suffix: String) =
        exec("data.set_serial", "name" to name, "start" to start, "step" to step, "digits" to digits, "prefix" to prefix, "suffix" to suffix)

    fun removeSerial(name: String) = exec("data.remove_serial", "name" to name)

    fun setVariable(name: String, value: String) = exec("variable.set", "name" to name, "value" to value)

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

    /**
     * [sheet] picks a sheet of a spreadsheet; with [header] false the first row is data and the
     * columns are named A, B, C….
     */
    suspend fun attachDataNow(name: String, bytes: ByteArray, sheet: String? = null, header: Boolean = true) {
        if (isSpreadsheet(name, bytes)) {
            // The engine reads files only inside its data dir; the rows are embedded, so the copy can go.
            val file = File(app.importsDir, exportBaseName(name))
            try {
                withContext(Dispatchers.IO) {
                    app.importsDir.mkdirs()
                    file.writeBytes(bytes)
                }
                val r = engine.call("data.attach_xlsx", "doc_id" to docId, "path" to file.path, "sheet" to sheet, "header" to header)
                sheets = r.optJSONArray("sheets").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) } }
                this.sheet = r.optString("sheet").ifEmpty { null }
            } finally {
                withContext(Dispatchers.IO) { file.delete() }
            }
            pickedName = null
        } else {
            engine.call("data.attach_csv", "doc_id" to docId, "text" to decodeText(bytes), "header" to header)
            pickedName = name
            sheets = emptyList()
            this.sheet = null
        }
        this.header = header
        pickedBytes = bytes
        pickedFile = name
        record = 1
        rows = emptyList()
        changed()
    }

    private var pickedFile: String? = null

    /** Whether the picked file is still at hand, so its sheet and header row can be changed. */
    val canReread: Boolean get() = pickedBytes != null && data != null

    /** Read the picked file again with another [sheet] or [header] setting. */
    fun reread(sheet: String? = this.sheet, header: Boolean = this.header) = app.scope.launch {
        val bytes = pickedBytes ?: return@launch
        app.report { attachDataNow(pickedFile.orEmpty(), bytes, sheet, header) }
    }

    /**
     * One record per picture (`data.attach_images`): the pictures go through the engine's data
     * dir, and an image element bound to the column is placed when the label has none.
     */
    fun attachImages(pictures: List<Pair<String, ByteArray>>) = app.scope.launch {
        if (pictures.isEmpty()) return@launch
        app.report {
            val files = pictures.mapIndexed { i, (name, _) -> File(app.importsDir, "%03d-%s".format(i + 1, exportBaseName(name))) }
            try {
                withContext(Dispatchers.IO) {
                    app.importsDir.mkdirs()
                    files.forEachIndexed { i, f -> f.writeBytes(pictures[i].second) }
                }
                engine.call("data.attach_images", "doc_id" to docId, "paths" to JSONArray(files.map { it.path }))
            } finally {
                withContext(Dispatchers.IO) { files.forEach { it.delete() } }
            }
            forgetPicked()
            record = 1
            rows = emptyList()
            changed()
        }
    }

    private fun forgetPicked() {
        pickedName = null
        pickedBytes = null
        pickedFile = null
        sheets = emptyList()
        sheet = null
    }

    fun detachData(): Job {
        forgetPicked()
        return exec("data.detach")
    }

    /** A text element that shows [column] of each record. */
    fun addBoundText(column: String) = addText("{{$column}}")

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

    /** `doc.set_label` with any of name, corner_radius_mm, printer_model (`""` clears) and density (0 clears). */
    fun setLabelProps(vararg props: Pair<String, Any?>) = exec("doc.set_label", *props)

    /** Use a paper of the library for this label (`paper.apply`). */
    fun applyPaper(id: String) = exec("paper.apply", "id" to id)

    /**
     * Read the loaded roll and compare it with this label. With "use the detected paper
     * automatically" on, a recognised roll of another size becomes the label's paper.
     */
    fun detectPaper() = app.scope.launch {
        val d = app.detectPaper(docId) ?: return@launch
        val paper = d.paper
        if (app.autoApplyPaper && paper != null && d.matchesLabel == false) {
            applyPaper(paper.id).join()
            app.detectPaper(docId)
        }
    }

    /** Save into [folder] of the library; an existing template of that name is replaced. */
    fun saveTemplate(name: String, folder: String = "") = app.scope.launch {
        app.report {
            engine.call("template.save", "doc_id" to docId, "name" to name, "folder" to folder.ifEmpty { null }, "overwrite" to true)
            app.thumbs.remove(if (folder.isEmpty()) "my:$name" else "my:$folder/$name")
            app.loadTemplates()
        }
    }

    /** The saved `.labely` file's content, for "Save a copy". */
    suspend fun fileBytes(): ByteArray {
        save()
        val path = doc?.path ?: error("the label is not saved")
        return withContext(Dispatchers.IO) { File(path).readBytes() }
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
    fun print(copies: Int, density: Int?, all: Boolean = false) = print(
        PrintOptions(
            copies = copies,
            density = density,
            records = if (recordCount > 0) (if (all) "all" else record.toString()) else null,
            total = copies * if (all) recordCount.coerceAtLeast(1) else 1,
        ),
    )

    /** Stop the running print at the next label (`job.cancel`). */
    fun cancelPrint() {
        val id = jobId ?: return
        app.cancelJob(id)
    }

    fun print(o: PrintOptions) {
        val printer = app.printer ?: return
        printJob?.cancel()
        val total = o.total
        jobId = null
        progress = PrintProgress(0, total, "queued", null)
        printJob = app.scope.launch {
            app.report {
                try {
                    val r = engine.call(
                        "print.submit",
                        "doc_id" to docId, "address" to printer.address, "copies" to o.copies, "density" to o.density, "records" to o.records,
                        "copies_column" to o.copiesColumn,
                        "count" to o.count,
                        "offset_x_mm" to o.offsetXMm.takeIf { it != 0.0 }, "offset_y_mm" to o.offsetYMm.takeIf { it != 0.0 },
                        "mirror" to o.mirror.takeIf { it }, "rotate_180" to o.rotate180.takeIf { it },
                        "grayscale" to o.grayscale.takeIf { it },
                        "wait" to false,
                    )
                    val jobId = r.getString("job_id")
                    this@EditorState.jobId = jobId
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

        const val DATE_BINDING = "{{now:%Y-%m-%d}}"
        const val TIME_BINDING = "{{now:%H:%M}}"
        const val SERIAL_BINDING = "{{serial}}"
    }
}

/** The rich-text ranges of a text element. */
fun spansOf(e: Element): List<Span> =
    e.raw.optJSONArray("spans").objects().map {
        Span(
            it.optInt("start"),
            it.optInt("end"),
            if (it.has("bold") && !it.isNull("bold")) it.optBoolean("bold") else null,
            if (it.has("italic") && !it.isNull("italic")) it.optBoolean("italic") else null,
            if (it.has("underline") && !it.isNull("underline")) it.optBoolean("underline") else null,
            if (it.has("font_size_pt") && !it.isNull("font_size_pt")) it.optDouble("font_size_pt") else null,
        )
    }
