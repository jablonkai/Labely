package org.labely.app

import android.content.Context
import android.content.pm.ApplicationInfo
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.core.content.edit
import java.io.File
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import org.labely.app.editor.EditorState
import org.labely.app.editor.Units
import org.labely.app.ui.theme.Appearance
import org.labely.app.engine.Detected
import org.labely.app.engine.JobInfo
import org.labely.app.engine.CommandFailure
import org.labely.app.engine.Device
import org.labely.app.engine.DeviceStatus
import org.labely.app.engine.Engine
import org.labely.app.engine.HistoryEntry
import org.labely.app.engine.PrinterSettings
import org.labely.app.engine.json
import org.labely.app.engine.Template
import org.labely.app.engine.objects
import org.labely.app.ui.templates.displayName

enum class Tab { Home, Templates, History, Printers, Settings }

/** A document the user opened or saved recently. */
data class Recent(val path: String, val name: String, val widthMm: Double, val heightMm: Double)

/** A printer the user connected to before, remembered across launches. */
data class KnownPrinter(val address: String, val name: String)

/** Label sizes offered on the home screen and in the new-label sheet, in millimetres. */
val PRESET_SIZES = listOf(50.0 to 30.0, 40.0 to 30.0, 30.0 to 15.0, 50.0 to 50.0, 40.0 to 12.0, 70.0 to 40.0)

/** The model new labels are made for until a printer is connected. */
const val DEFAULT_MODEL = "niimbot.b21_pro"

/** The B21 Pro simulator, offered in debug builds so the app works without a printer. */
const val SIM_ADDRESS = "sim:niimbot.b21_pro"

/**
 * Everything the screens show, read from the engine. It outlives the activity (it lives in
 * [LabelyApplication]), so rotating the phone keeps the open document.
 */
class AppState(private val context: Context, val engine: Engine, val scope: CoroutineScope) {
    private val prefs = context.getSharedPreferences("labely", Context.MODE_PRIVATE)
    /** Where labels are saved; AppFunctions address labels by their file name here. */
    val labelsDir = File(context.filesDir, "labels").apply { mkdirs() }

    /** Inside the engine's data dir, the only place it reads and writes: picked spreadsheets and exports. */
    val importsDir = File(context.filesDir, "imports")
    val exportsDir = File(context.filesDir, "exports")
    val debug = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    var tab by mutableStateOf(Tab.Home)

    // ---- preferences ----

    private inner class BoolPref(private val key: String, default: Boolean) {
        private var state by mutableStateOf(prefs.getBoolean(key, default))

        operator fun getValue(owner: Any?, p: kotlin.reflect.KProperty<*>): Boolean = state

        operator fun setValue(owner: Any?, p: kotlin.reflect.KProperty<*>, v: Boolean) {
            state = v
            prefs.edit { putBoolean(key, v) }
        }
    }

    /** Lengths are shown in millimetres or inches; the engine always gets millimetres. */
    private var unitsState by mutableStateOf(Units.of(prefs.getString("units", null)))
    var units: Units
        get() = unitsState
        set(u) {
            unitsState = u
            org.labely.app.ui.Lengths.units = u
            prefs.edit { putString("units", u.id) }
        }

    /** Settings → Appearance: follow the system, or always light or dark. */
    private var appearanceState by mutableStateOf(Appearance.of(prefs.getString("appearance", null)))
    var appearance: Appearance
        get() = appearanceState
        set(a) {
            appearanceState = a
            prefs.edit { putString("appearance", a.id) }
        }

    /** Canvas aids, kept across documents like the studio's view settings. */
    var showGrid by BoolPref("grid", false)
    var showRulers by BoolPref("rulers", false)
    var snap by BoolPref("snap", true)
    var rotateSnap by BoolPref("rotate_snap", true)

    /** Use the paper the printer detects for the open label without asking. */
    var autoApplyPaper by BoolPref("auto_apply_paper", false)

    private var gridState by mutableStateOf(prefs.getFloat("grid_mm", 1f).toDouble())
    var gridMm: Double
        get() = gridState
        set(v) {
            gridState = v.coerceIn(0.1, 50.0)
            prefs.edit { putFloat("grid_mm", gridState.toFloat()) }
        }

    /** The colour the label is shown in on screen (ARGB); printing is not affected. */
    private var labelColorState by mutableStateOf(prefs.getInt("label_color", -1))
    var labelColor: Int
        get() = labelColorState
        set(v) {
            labelColorState = v
            prefs.edit { putInt("label_color", v) }
        }

    /** The print offset last used with a printer, in mm (x, y). */
    fun printOffset(address: String): Pair<Double, Double> =
        prefs.getFloat("offset_x:$address", 0f).toDouble() to prefs.getFloat("offset_y:$address", 0f).toDouble()

    fun setPrintOffset(address: String, x: Double, y: Double) {
        prefs.edit {
            putFloat("offset_x:$address", x.toFloat())
            putFloat("offset_y:$address", y.toFloat())
        }
    }

    /** Elements copied in an editor (`{elements, assets}`), pasted with `element.paste` into any document. */
    var clip by mutableStateOf<JSONObject?>(null)

    /** Plain text on the system clipboard, if any. */
    fun systemClipboard(): String? =
        runCatching {
            context.getSystemService(android.content.ClipboardManager::class.java)?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
        }.getOrNull()?.takeIf { it.isNotBlank() }

    fun setSystemClipboard(text: String) {
        runCatching {
            context.getSystemService(android.content.ClipboardManager::class.java)?.setPrimaryClip(android.content.ClipData.newPlainText("Labely", text))
        }
    }

    /** Set when a document should open with the print sheet showing ("Print again"). */
    var printOnOpen by mutableStateOf(false)

    /** For "Continue": labels already printed, labels in all and copies of the job that stopped. */
    var resume by mutableStateOf<Triple<Int, Int, Int>?>(null)

    /** Set when a new label should open with the data panel showing ("Labels from data"). */
    var dataOnOpen by mutableStateOf(false)

    /** Library folders besides the default one. */
    var folders by mutableStateOf<List<String>>(emptyList())
        private set

    /** Font families the engine can draw. */
    var fonts by mutableStateOf<List<String>>(emptyList())
        private set
    private val fontsDir = File(context.filesDir, "fonts")

    /** Printer model ids the engine knows; see [loadModels]. */
    var modelIds by mutableStateOf<List<String>>(emptyList())
        private set

    /** The print queue: queued, running and finished jobs of this session, oldest first. */
    var jobs by mutableStateOf<List<JobInfo>>(emptyList())
        private set
    var queuePaused by mutableStateOf(false)
        private set

    /** What the connected printer's RFID reader says about the loaded roll; null until read. */
    var detected by mutableStateOf<Detected?>(null)
        private set

    /** The last connection that failed: address, name and the engine's message. */
    var connectError by mutableStateOf<Triple<String, String, String>?>(null)
    var templates by mutableStateOf<List<Template>>(emptyList())
        private set
    val thumbs = mutableStateMapOf<String, ImageBitmap>()
    var recents by mutableStateOf(loadRecents())
        private set
    var known by mutableStateOf(loadKnown())
        private set
    var history by mutableStateOf<List<HistoryEntry>>(emptyList())
        private set

    /** The connected printer and its last status. */
    var printer by mutableStateOf<Device?>(null)
        private set
    var printerStatus by mutableStateOf<DeviceStatus?>(null)
        private set

    /** What the connected printer reports of its settings; null until read. */
    var printerSettings by mutableStateOf<PrinterSettings?>(null)
        private set

    /** Address being connected, or [SCANNING] while scanning. */
    var busy by mutableStateOf<String?>(null)
        private set
    var scanResults by mutableStateOf<List<Device>?>(null)
        private set

    /** Whether assistants may print through AppFunctions (ADR-0028). Off until the user allows it. */
    private var agentPrint by mutableStateOf(prefs.getBoolean("allow_agent_print", false))
    var allowAgentPrint: Boolean
        get() = agentPrint
        set(allow) {
            agentPrint = allow
            prefs.edit { putBoolean("allow_agent_print", allow) }
        }

    /** One-line message for the snackbar. */
    var message by mutableStateOf<String?>(null)

    var editor by mutableStateOf<EditorState?>(null)
        private set

    /** The model new documents and checks use. */
    val modelId: String get() = printer?.model?.id ?: DEFAULT_MODEL

    /** Known models by id: all of the engine's once [loadModels] ran, before that the ones connected so far. */
    private var seenModels by mutableStateOf(loadModelsSeen())

    /** What is known of model [id]: its name, resolution and printhead width. */
    fun model(id: String?): org.labely.app.engine.Model? = if (id == null) null else printer?.model?.takeIf { it.id == id } ?: seenModels[id]

    private fun sawModel(m: org.labely.app.engine.Model) {
        if (seenModels[m.id] == m) return
        seenModels = seenModels + (m.id to m)
        prefs.edit {
            putString(
                "models",
                JSONArray(
                    seenModels.values.map {
                        JSONObject().put("id", it.id).put("name", it.name).put("density_min", it.densityMin).put("density_max", it.densityMax)
                            .put("density_default", it.densityDefault).put("dpi", it.dpi).put("head_dots", it.headDots).put("max_width_mm", it.maxWidthMm)
                            .put("gray", it.gray).put("red", it.red)
                    },
                ).toString(),
            )
        }
    }

    private fun loadModelsSeen(): Map<String, org.labely.app.engine.Model> =
        runCatching {
            JSONArray(prefs.getString("models", "[]")).objects().associate {
                it.getString("id") to org.labely.app.engine.Model(
                    it.getString("id"), it.optString("name"), it.optInt("density_min", 1), it.optInt("density_max", 5), it.optInt("density_default", 3),
                    it.optInt("dpi", 203), it.optInt("head_dots"), it.optDouble("max_width_mm", 0.0), it.optBoolean("gray"), it.optBoolean("red"),
                )
            }
        }.getOrDefault(emptyMap())

    fun start() {
        org.labely.app.ui.Lengths.units = units
        scope.launch { engine.devices.collect(::onDeviceEvent) }
        scope.launch { engine.allJobs.collect { m -> jobs = m.values.map(JobInfo::from) } }
        scope.launch { loadFonts() }
        scope.launch {
            loadTemplates()
            refreshHistory()
        }
    }

    /** A printer event pushed by the engine (ADR-0031): a dropped link or a new status. */
    private fun onDeviceEvent(e: JSONObject) {
        val p = printer ?: return
        if (e.optString("address") != p.address) return
        if (!e.optBoolean("connected")) {
            printer = null
            printerStatus = null
            detected = null
            if (e.optBoolean("lost")) message = context.getString(R.string.printer_lost)
        } else if (e.has("status")) {
            printerStatus = DeviceStatus.from(e)
        }
    }

    // ---- templates ----

    suspend fun loadTemplates() {
        report {
            // With the loaded roll, the templates that fit it come first (`fits`).
            val r = engine.call("template.list", "paper" to detected?.paper?.id)
            templates = Template.list(r)
            folders = r.optJSONArray("folders").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) } }
            for (t in templates) if (t.ref !in thumbs) thumbnail(t.ref) { engine.call("template.new", "ref" to t.ref) }
            for (r in recents) if (r.path !in thumbs) thumbnail(r.path) { engine.call("doc.open", "path" to r.path) }
        }
    }

    /** Open a document with [open], render it small, close it. Failures leave no thumbnail. */
    private suspend fun thumbnail(key: String, open: suspend () -> JSONObject) {
        runCatching {
            val docId = open().getString("doc_id")
            try {
                thumbs[key] = engine.render(THUMB_DPI, docId)
            } finally {
                engine.call("doc.close", "doc_id" to docId)
            }
        }
    }

    fun deleteTemplate(t: Template) = scope.launch {
        report {
            engine.call("template.delete", "ref" to t.ref)
            thumbs.remove(t.ref)
            loadTemplates()
        }
    }

    /** Copy [t] (a starter or a library template) into the library as [name] in [folder]. */
    fun duplicateTemplate(t: Template, name: String? = null, folder: String? = null) = scope.launch {
        report {
            engine.call("template.duplicate", "ref" to t.ref, "name" to name, "folder" to folder)
            loadTemplates()
        }
    }

    /** Rename a library template and/or move it to another folder (`""` is the default folder). */
    fun moveTemplate(t: Template, name: String? = null, folder: String? = null) = scope.launch {
        report {
            engine.call("template.move", "ref" to t.ref, "name" to name, "folder" to folder)
            thumbs.remove(t.ref)
            loadTemplates()
        }
    }

    /** `template.folder`: [op] is `create`, `rename` (to [to]) or `delete`. */
    fun templateFolder(op: String, folder: String, to: String? = null, withTemplates: Boolean? = null) = scope.launch {
        report {
            engine.call("template.folder", "op" to op, "folder" to folder, "to" to to, "with_templates" to withTemplates)
            loadTemplates()
        }
    }

    /** Whether the library already has a template [name] in [folder]. */
    fun templateExists(name: String, folder: String): Boolean = templates.any { !it.starter && it.name == name && it.folder == folder }

    // ---- icons ----

    /** The built-in icons (`icon.list`) as name to category; read once. */
    var icons by mutableStateOf<List<Pair<String, String>>>(emptyList())
        private set

    suspend fun loadIcons() {
        if (icons.isNotEmpty()) return
        report { icons = engine.call("icon.list").optJSONArray("icons").objects().map { it.optString("name") to it.optString("category") } }
    }

    /** A small raster of icon [name], drawn by the engine on a scratch label; kept in [thumbs]. */
    suspend fun iconThumb(name: String): ImageBitmap? {
        val key = "icon:$name"
        thumbs[key]?.let { return it }
        return runCatching {
            val docId = engine.call("doc.new", "width_mm" to 10.0, "height_mm" to 10.0).getString("doc_id")
            try {
                engine.call("element.add_icon", "doc_id" to docId, "x_mm" to 0.5, "y_mm" to 0.5, "width_mm" to 9.0, "height_mm" to 9.0, "icon" to name)
                engine.render(THUMB_DPI, docId).also { thumbs[key] = it }
            } finally {
                engine.call("doc.close", "doc_id" to docId)
            }
        }.getOrNull()
    }

    // ---- fonts ----

    /** Load the user's font files again (the engine keeps them for one session) and list the families. */
    suspend fun loadFonts() {
        report {
            for (f in fontsDir.listFiles().orEmpty()) runCatching { engine.call("font.add", "path" to f.path) }
            fonts = engine.call("font.list").optJSONArray("fonts").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) } }
        }
    }

    /** Add a picked font file; returns the families it brought, empty when it is not a font. */
    suspend fun addFont(name: String, bytes: ByteArray): List<String> {
        val file = File(fontsDir, org.labely.app.editor.exportBaseName(name))
        withContext(Dispatchers.IO) {
            fontsDir.mkdirs()
            file.writeBytes(bytes)
        }
        try {
            val r = engine.call("font.add", "path" to file.path)
            fonts = r.optJSONArray("fonts").let { a -> if (a == null) fonts else (0 until a.length()).map { a.optString(it) } }
            return r.optJSONArray("added").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) } }
        } catch (e: CommandFailure) {
            withContext(Dispatchers.IO) { file.delete() }
            throw e
        }
    }

    // ---- printer models ----

    /** Load every printer model the engine knows (`device.models`), with name, dpi and head width. */
    suspend fun loadModels() {
        if (modelIds.isNotEmpty()) return
        val all = runCatching { engine.call("device.models").getJSONArray("models").objects() }.getOrDefault(emptyList()).map {
            org.labely.app.engine.Model(
                it.getString("id"), it.optString("name"), it.optInt("density_min", 1), it.optInt("density_max", 5), it.optInt("density_default", 3),
                it.optInt("dpi", 203), it.optInt("head_dots"), it.optDouble("max_width_mm", 0.0), it.optBoolean("gray"), it.optBoolean("red"),
            )
        }
        seenModels = seenModels + all.associateBy { it.id }
        modelIds = all.map { it.id }.sorted()
    }

    // ---- documents ----

    fun newLabel(widthMm: Double, heightMm: Double, media: String = "gap") = openWith {
        engine.call(
            "doc.new",
            "width_mm" to widthMm,
            "height_mm" to heightMm,
            "media" to media,
            "printer_model" to modelId,
            "name" to context.getString(R.string.new_label_name),
        )
    }

    /** "Labels from data": a new label that opens on the data panel, ready for a file. */
    fun newLabelFromData() = newLabel(PRESET_SIZES[0].first, PRESET_SIZES[0].second).also { dataOnOpen = true }

    fun fromTemplate(t: Template) = openWith { engine.call("template.new", "ref" to t.ref, "name" to t.displayName(context)) }

    fun openRecent(r: Recent) = openWith { engine.call("doc.open", "path" to r.path) }

    fun openHistory(h: HistoryEntry, print: Boolean = false) = openWith {
        engine.call("job.history_open", "id" to h.id).also {
            printOnOpen = print
            // A job that stopped part-way continues after the labels that came out whole.
            resume = if (print && h.state != "done" && h.copies > 0) Triple(h.pages / h.copies, h.labels, h.copies) else null
        }
    }

    /** Run [edit] on the saved document of [r] and save it to [target]. */
    private suspend fun editFile(r: Recent, target: String, edit: suspend (String) -> Unit): ImageBitmap {
        val docId = engine.call("doc.open", "path" to r.path).getString("doc_id")
        try {
            edit(docId)
            engine.call("doc.save", "doc_id" to docId, "path" to target)
            return engine.render(THUMB_DPI, docId)
        } finally {
            engine.call("doc.close", "doc_id" to docId)
        }
    }

    /** A copy of a recent label under [name], as a new file. */
    fun duplicateRecent(r: Recent, name: String) = scope.launch {
        report {
            val path = newDocPath()
            val thumb = editFile(r, path) { engine.call("doc.set_label", "doc_id" to it, "name" to name) }
            remember(Recent(path, name, r.widthMm, r.heightMm), thumb)
        }
    }

    fun renameRecent(r: Recent, name: String) = scope.launch {
        report {
            editFile(r, r.path) { engine.call("doc.set_label", "doc_id" to it, "name" to name) }
            recents = recents.map { if (it.path == r.path) it.copy(name = name) else it }
            saveRecents()
        }
    }

    /** Delete a recent label's file. */
    fun deleteRecent(r: Recent) = scope.launch {
        report {
            withContext(Dispatchers.IO) { File(r.path).delete() }
            thumbs.remove(r.path)
            recents = recents.filter { it.path != r.path }
            saveRecents()
        }
    }

    /** A new label of the paper the printer detected, or of [widthMm] × [heightMm] with data attached later. */
    fun newLabelFromPaper(paperId: String) = openWith {
        val r = engine.call("doc.new", "width_mm" to 50.0, "height_mm" to 30.0, "printer_model" to modelId, "name" to context.getString(R.string.new_label_name))
        engine.call("paper.apply", "doc_id" to r.getString("doc_id"), "id" to paperId)
        r
    }

    /** Copy a `.labely` file the user picked into app storage and open it. */
    fun openUri(uri: Uri) = openWith {
        val target = File(labelsDir, "${stamp()}.labely")
        withContext(Dispatchers.IO) {
            context.contentResolver.openInputStream(uri)?.use { input -> target.outputStream().use { input.copyTo(it) } }
                ?: error("cannot read $uri")
        }
        engine.call("doc.open", "path" to target.path)
    }

    private fun openWith(open: suspend () -> JSONObject) = scope.launch {
        report {
            val docId = open().getString("doc_id")
            val ed = EditorState(this@AppState, docId)
            ed.refresh()
            editor = ed
        }
    }

    /** Close the editor's document. Edits were saved as they happened; an untouched one is dropped. */
    fun closeEditor() {
        val ed = editor ?: return
        editor = null
        scope.launch {
            report {
                if (!ed.saved) ed.save()
                engine.call("doc.close", "doc_id" to ed.docId)
            }
        }
    }

    /** Where a document without a path is saved. */
    fun newDocPath(): String = File(labelsDir, "${stamp()}.labely").path

    fun remember(r: Recent, preview: ImageBitmap?) {
        recents = (listOf(r) + recents.filter { it.path != r.path }).take(MAX_RECENTS)
        if (preview != null) thumbs[r.path] = preview
        saveRecents()
    }

    private fun saveRecents() {
        prefs.edit {
            putString(
                "recents",
                JSONArray(recents.map { JSONObject().put("path", it.path).put("name", it.name).put("w", it.widthMm).put("h", it.heightMm) }).toString(),
            )
        }
    }

    private fun loadRecents(): List<Recent> =
        runCatching {
            JSONArray(prefs.getString("recents", "[]")).objects()
                .map { Recent(it.getString("path"), it.optString("name"), it.optDouble("w"), it.optDouble("h")) }
                .filter { File(it.path).exists() }
        }.getOrDefault(emptyList())

    // ---- history ----

    suspend fun refreshHistory() {
        report { history = HistoryEntry.list(engine.call("job.history", "limit" to 200)) }
    }

    fun deleteHistory(h: HistoryEntry) = scope.launch {
        report { engine.call("job.history_delete", "ids" to JSONArray(listOf(h.id))) }
        refreshHistory()
    }

    fun clearHistory() = scope.launch {
        report { engine.call("job.history_clear") }
        refreshHistory()
    }

    // ---- print queue ----

    /** Read the queue (`job.list`): whether it is paused, and its jobs. */
    suspend fun refreshJobs() {
        report {
            val r = engine.call("job.list")
            queuePaused = r.optBoolean("paused")
            jobs = r.optJSONArray("jobs").objects().map(JobInfo::from)
        }
    }

    fun cancelJob(id: String) = scope.launch { report { engine.call("job.cancel", "id" to id) } }

    /** A paused queue finishes the running job and starts no new one. */
    fun pauseQueue(pause: Boolean) = scope.launch {
        report { queuePaused = engine.call(if (pause) "job.pause" else "job.resume").optBoolean("paused") }
    }

    /** Drop the finished jobs from the queue view. */
    fun clearJobs() = scope.launch {
        report { engine.call("job.clear") }
        refreshJobs()
    }

    // ---- printers ----

    /** Scan for printers. Bluetooth permissions must already be granted. */
    fun scan() = scope.launch {
        busy = SCANNING
        try {
            report {
                val found = engine.call("device.scan", "timeout_ms" to SCAN_MS).optJSONArray("devices").objects().map(Device::from)
                scanResults = found.filter { it.address.startsWith("ble:") }
            }
        } finally {
            busy = null
        }
    }

    fun connect(address: String, name: String) = scope.launch {
        busy = address
        connectError = null
        try {
            connectNow(address, name)
        } catch (e: CommandFailure) {
            // Shown with tips and "Try again" on the printers screen.
            connectError = Triple(address, name, listOfNotNull(e.message?.ifEmpty { e.code } ?: e.code, e.hint).joinToString(" · "))
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            connectError = Triple(address, name, e.message ?: e.toString())
        } finally {
            busy = null
        }
    }

    /**
     * Connect to [address] unless it is already the connected printer, and make it the app's
     * printer. Throws [CommandFailure]; used by the UI and by AppFunctions.
     */
    suspend fun connectNow(address: String, name: String = ""): Device {
        printer?.let { if (it.address == address) return it }
        printer?.let { disconnectQuietly(it.address) }
        val dev = Device.from(engine.call("device.connect", "address" to address))
        printer = dev
        dev.model?.let(::sawModel)
        known = (listOf(KnownPrinter(address, name.ifEmpty { dev.name })) + known.filter { it.address != address }).take(MAX_KNOWN)
        saveKnown()
        refreshStatus()
        return dev
    }

    fun disconnect() = scope.launch {
        val p = printer ?: return@launch
        report { disconnectQuietly(p.address) }
    }

    private suspend fun disconnectQuietly(address: String) {
        runCatching { engine.call("device.disconnect", "address" to address) }
        printer = null
        printerStatus = null
        val hadRoll = detected?.paper != null
        detected = null
        if (hadRoll) loadTemplates()
    }

    suspend fun refreshStatus() {
        val p = printer ?: return
        try {
            printerStatus = DeviceStatus.from(engine.call("device.status", "address" to p.address))
        } catch (e: CommandFailure) {
            if (e.code == "device_unavailable" || e.code == "not_found") {
                printer = null
                printerStatus = null
            }
        }
    }

    /**
     * Ask the printer which roll is loaded (`paper.detect`) and compare it with [docId]'s label.
     * A printer without an RFID reader answers `rfid: false`.
     */
    suspend fun detectPaper(docId: String? = null): Detected? {
        val p = printer ?: return null
        return try {
            Detected.from(engine.call("paper.detect", "address" to p.address, "doc_id" to docId)).also {
                val rollChanged = it.paper?.id != detected?.paper?.id
                detected = it
                if (rollChanged) loadTemplates()
            }
        } catch (e: CommandFailure) {
            detected = null
            null
        }
    }

    /** Read the connected printer's settings (`device.settings`). */
    suspend fun loadSettings() {
        printerSettings = null
        val p = printer ?: return
        report { printerSettings = PrinterSettings.from(engine.call("device.settings", "address" to p.address)) }
    }

    /** Change settings, e.g. `"auto_off_level" to 2`; the printer's answer becomes [printerSettings]. */
    fun setSettings(vararg change: Pair<String, Any?>) = scope.launch { report { setSettingsNow(*change) } }

    suspend fun setSettingsNow(vararg change: Pair<String, Any?>) {
        val p = printer ?: return
        printerSettings = PrinterSettings.from(engine.call("device.set_settings", json("address" to p.address, *change)))
    }

    fun calibrate() = scope.launch {
        val p = printer ?: return@launch
        busy = p.address
        try {
            report {
                engine.call("device.calibrate", "address" to p.address)
                message = context.getString(R.string.calibrated)
            }
        } finally {
            busy = null
        }
    }

    fun forget(k: KnownPrinter) {
        known = known.filter { it.address != k.address }
        saveKnown()
    }

    private fun saveKnown() {
        prefs.edit {
            putString("known", JSONArray(known.map { JSONObject().put("address", it.address).put("name", it.name) }).toString())
        }
    }

    private fun loadKnown(): List<KnownPrinter> =
        runCatching {
            JSONArray(prefs.getString("known", "[]")).objects().map { KnownPrinter(it.getString("address"), it.optString("name")) }
        }.getOrDefault(emptyList())

    // ---- errors ----

    /** Run [block]; a failure becomes the snackbar message instead of a crash. */
    suspend fun report(block: suspend () -> Unit) {
        try {
            block()
        } catch (e: CommandFailure) {
            message = e.message?.ifEmpty { e.code } ?: e.code
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            message = e.message ?: e.toString()
        }
    }

    companion object {
        const val SCANNING = "scan"
        const val THUMB_DPI = 203
        private const val SCAN_MS = 5000
        private const val MAX_RECENTS = 12
        private const val MAX_KNOWN = 8

        private fun stamp(): String = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyyMMdd-HHmmss-SSS"))
    }
}
