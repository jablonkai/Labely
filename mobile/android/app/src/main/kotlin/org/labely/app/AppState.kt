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
import org.labely.app.engine.CommandFailure
import org.labely.app.engine.Device
import org.labely.app.engine.DeviceStatus
import org.labely.app.engine.Engine
import org.labely.app.engine.HistoryEntry
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
    val debug = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    var tab by mutableStateOf(Tab.Home)
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

    fun start() {
        scope.launch {
            loadTemplates()
            refreshHistory()
        }
    }

    // ---- templates ----

    suspend fun loadTemplates() {
        report {
            templates = Template.list(engine.call("template.list"))
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

    fun fromTemplate(t: Template) = openWith { engine.call("template.new", "ref" to t.ref, "name" to t.displayName(context)) }

    fun openRecent(r: Recent) = openWith { engine.call("doc.open", "path" to r.path) }

    fun openHistory(h: HistoryEntry) = openWith { engine.call("job.history_open", "id" to h.id) }

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
        report { history = HistoryEntry.list(engine.call("job.history", "limit" to 50)) }
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
        try {
            report { connectNow(address, name) }
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
