package org.labely.app.appfunctions

import androidx.annotation.RequiresApi
import androidx.appfunctions.AppFunctionAppUnknownException
import androidx.appfunctions.AppFunctionDeclaration
import androidx.appfunctions.AppFunctionDeniedException
import androidx.appfunctions.AppFunctionElementNotFoundException
import androidx.appfunctions.AppFunctionException
import androidx.appfunctions.AppFunctionInvalidArgumentException
import androidx.appfunctions.AppFunctionNotSupportedException
import androidx.appfunctions.AppFunctionPermissionRequiredException
import androidx.appfunctions.AppFunctionService
import androidx.appfunctions.AppFunctionServiceEntryPoint
import java.io.File
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import org.labely.app.AppState
import org.labely.app.LabelyApplication
import org.labely.app.R
import org.labely.app.Recent
import org.labely.app.SIM_ADDRESS
import org.labely.app.engine.CommandFailure
import org.labely.app.engine.Device
import org.labely.app.engine.DeviceStatus
import org.labely.app.engine.Doc
import org.labely.app.engine.Element
import org.labely.app.engine.Issue
import org.labely.app.engine.Template
import org.labely.app.engine.objects
import org.labely.app.ui.templates.displayName

/**
 * The six AppFunctions of ADR-0011 (ADR-0028). Each one is a few engine commands on the app's
 * own engine and state, so what an assistant makes shows up in the app. Labels are saved to the
 * app's label folder and addressed by file name, so a label id survives the process.
 *
 * The functions run on the main thread; [org.labely.app.engine.Engine] moves every command to
 * its worker thread.
 */
@RequiresApi(36)
@AppFunctionServiceEntryPoint(
    serviceName = "LabelyAppFunctionService",
    appFunctionXmlFileName = "labely_app_functions",
)
abstract class LabelyFunctions : AppFunctionService() {
    private val app: AppState get() = (applicationContext as LabelyApplication).state

    /**
     * Create a label that shows a text and save it, ready to print. The text is centred and
     * shrinks to fit the label.
     *
     * Next step: call "printLabel" with the returned labelId to print it.
     *
     * @param text Text to print. Line breaks start new lines. Must not be empty.
     * @param widthMm Label width in millimetres, 10 to 120. 0 uses 50 mm.
     * @param heightMm Label height in millimetres, 5 to 120. 0 uses 30 mm.
     * @return The saved label and any print warnings.
     * @throws AppFunctionInvalidArgumentException If the text is empty or a size is out of range.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun createLabelFromText(text: String, widthMm: Double = 0.0, heightMm: Double = 0.0): SavedLabel =
        mapped {
            if (text.isBlank()) throw AppFunctionInvalidArgumentException("text is empty")
            val w = widthMm.takeIf { it > 0 } ?: DEFAULT_WIDTH_MM
            val h = heightMm.takeIf { it > 0 } ?: DEFAULT_HEIGHT_MM
            if (w !in 10.0..120.0 || h !in 5.0..120.0) {
                throw AppFunctionInvalidArgumentException("label size ${w}x$h mm is out of range (10–120 × 5–120 mm)")
            }
            val name = text.lineSequence().first { it.isNotBlank() }.trim().take(NAME_CHARS)
            val docId = app.engine.call(
                "doc.new",
                "width_mm" to w, "height_mm" to h, "media" to "gap", "printer_model" to app.modelId, "name" to name,
            ).getString("doc_id")
            withDoc(docId) {
                val m = minOf(MARGIN_MM, minOf(w, h) / 10)
                app.engine.call(
                    "element.add_text",
                    "doc_id" to docId, "x_mm" to m, "y_mm" to m, "width_mm" to w - 2 * m, "height_mm" to h - 2 * m,
                    "text" to text.trim(), "font_size_pt" to TEXT_PT, "fit" to "shrink", "wrap" to true,
                    "align" to "center", "valign" to "middle",
                )
                save(docId)
            }
        }

    /**
     * Create a label from a template, fill in its fields and save it, ready to print. Fields that
     * are not given keep the template's sample value.
     *
     * Required workflow: call "listTemplates" first to get the templateId and its field names.
     * Next step: call "printLabel" with the returned labelId.
     *
     * @param templateId Template id from "listTemplates", such as "starter:price-tag".
     * @param fields New values by field name. An empty list keeps every sample value.
     * @return The saved label and any print warnings.
     * @throws AppFunctionElementNotFoundException If no template has this id; call "listTemplates".
     * @throws AppFunctionInvalidArgumentException If a field name is not on the template. The
     *   message lists the template's field names.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun createLabelFromTemplate(templateId: String, fields: List<FieldValue> = emptyList()): SavedLabel =
        mapped {
            val template = Template.list(app.engine.call("template.list")).find { it.ref == templateId }
                ?: throw AppFunctionElementNotFoundException("no template `$templateId`; call listTemplates")
            val docId = app.engine.call("template.new", "ref" to templateId, "name" to template.displayName(this))
                .getString("doc_id")
            withDoc(docId) {
                val elements = fieldElements(Doc.from(app.engine.call("doc.get", "doc_id" to docId)))
                for (f in fields) {
                    val targets = elements.filter { it.first == f.name }
                    if (targets.isEmpty()) {
                        throw AppFunctionInvalidArgumentException(
                            "template `$templateId` has no field `${f.name}`; its fields are " +
                                elements.map { it.first }.distinct().joinToString { "`$it`" },
                        )
                    }
                    for ((_, e) in targets) {
                        val key = if (e.type == "text") "text" else "data"
                        app.engine.call("element.set_props", "doc_id" to docId, "id" to e.id, "props" to JSONObject().put(key, f.value))
                    }
                }
                save(docId)
            }
        }

    /**
     * List the label templates: the starters that ship with Labely and the user's own, with the
     * fields each one can fill in.
     *
     * @param query Text to search template names for. Empty lists every template.
     * @return Templates, starters first.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun listTemplates(query: String? = null): List<LabelTemplate> =
        mapped {
            val list = Template.list(app.engine.call("template.list", "query" to query?.takeIf(String::isNotBlank)))
            list.map { t ->
                val docId = app.engine.call("template.new", "ref" to t.ref).getString("doc_id")
                val fields = withDoc(docId) { fieldElements(Doc.from(app.engine.call("doc.get", "doc_id" to docId))) }
                LabelTemplate(
                    templateId = t.ref,
                    name = t.displayName(this),
                    folder = t.folder,
                    widthMm = t.widthMm,
                    heightMm = t.heightMm,
                    fields = fields.distinctBy { it.first }.map { (name, e) ->
                        TemplateField(name, if (e.type == "text") "text" else e.type, e.text ?: e.data.orEmpty())
                    },
                )
            }
        }

    /**
     * List the label printers: the connected one and those the user connected to before.
     *
     * @param scan Also search for printers nearby over Bluetooth. It takes about 5 seconds and
     *   needs the app's Bluetooth permission.
     * @return Printers, connected first.
     * @throws AppFunctionPermissionRequiredException If scan is set and Bluetooth is off or not
     *   allowed. Ask the user to turn Bluetooth on and open Labely once to allow it.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun listPrinters(scan: Boolean = false): List<Printer> =
        mapped {
            val found = linkedMapOf<String, Printer>()
            for (d in app.engine.call("device.list").optJSONArray("devices").objects().map(Device::from)) {
                if (d.connected) found[d.address] = d.toPrinter()
            }
            for (k in app.known) found.getOrPut(k.address) { Printer(k.address, k.name, false, "") }
            if (app.debug) found.getOrPut(SIM_ADDRESS) { Printer(SIM_ADDRESS, getString(R.string.simulator), false, "") }
            if (scan) {
                val near = app.engine.call("device.scan", "timeout_ms" to SCAN_MS).optJSONArray("devices").objects().map(Device::from)
                for (d in near) if (d.address.startsWith("ble:")) found.getOrPut(d.address) { d.toPrinter() }
            }
            found.values.sortedByDescending { it.connected }
        }

    /**
     * Get a printer's battery and paper, connecting to it first if needed.
     *
     * Required workflow: call "listPrinters" first to get the printerId.
     *
     * @param printerId Printer id from "listPrinters".
     * @return Battery and paper state.
     * @throws AppFunctionAppUnknownException If the printer does not answer. Ask the user to
     *   switch it on and bring it close to the phone.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun getPrinterStatus(printerId: String): PrinterStatus =
        mapped {
            val dev = app.connectNow(printerId)
            val s = DeviceStatus.from(app.engine.call("device.status", "address" to printerId))
            PrinterStatus(
                printerId = dev.address,
                name = dev.name,
                model = dev.model?.name.orEmpty(),
                batteryPercent = s.batteryPercent ?: -1,
                paper = when (s.paperPresent) {
                    true -> "loaded"
                    false -> "empty"
                    null -> "unknown"
                },
            )
        }

    /**
     * Print a saved label and wait until it has printed.
     *
     * Required workflow: call "createLabelFromText" or "createLabelFromTemplate" first to get the
     * labelId. Printing uses paper, so confirm with the user before calling this.
     *
     * @param labelId Label id from "createLabelFromText" or "createLabelFromTemplate".
     * @param printerId Printer id from "listPrinters". Null uses the connected printer, else the
     *   only known one.
     * @param copies Number of copies, 1 to 999. 0 prints one copy.
     * @return The finished print job.
     * @throws AppFunctionPermissionRequiredException If the user has not allowed assistants to
     *   print. Ask the user to turn on "Allow assistants to print" in Labely's settings, once.
     * @throws AppFunctionElementNotFoundException If no label has this id.
     * @throws AppFunctionInvalidArgumentException If no printer is given and none is known; call
     *   "listPrinters" with scan set.
     * @throws AppFunctionAppUnknownException If the print failed, for example when the printer
     *   is out of paper. The message says why.
     */
    @AppFunctionDeclaration(isDescribedByKDoc = true)
    suspend fun printLabel(labelId: String, printerId: String? = null, copies: Int = 0): PrintResult =
        mapped {
            if (!app.allowAgentPrint) {
                throw AppFunctionPermissionRequiredException(
                    "the user has not allowed assistants to print; ask them to turn on " +
                        "\"${getString(R.string.settings_agent_print)}\" in Labely's settings",
                )
            }
            val n = if (copies == 0) 1 else copies
            if (n !in 1..MAX_COPIES) throw AppFunctionInvalidArgumentException("copies must be 1–$MAX_COPIES")
            val file = labelFile(labelId)
            val address = printerId?.takeIf(String::isNotBlank)
                ?: app.printer?.address
                ?: app.known.singleOrNull()?.address
                ?: throw AppFunctionInvalidArgumentException("no printer chosen; call listPrinters and pass a printerId")
            app.connectNow(address)
            val docId = app.engine.call("doc.open", "path" to file.path).getString("doc_id")
            val job = withDoc(docId) {
                val jobId = app.engine.call("print.submit", "doc_id" to docId, "address" to address, "copies" to n, "wait" to false)
                    .getString("job_id")
                withTimeoutOrNull(PRINT_TIMEOUT_MS) { finished(jobId) }
                    ?: throw AppFunctionAppUnknownException("job $jobId did not finish in time; the print queue may be paused")
            }
            app.refreshHistory()
            app.refreshStatus()
            val state = job.optString("state")
            if (state != "done") {
                val why = job.optJSONObject("error")?.optString("message").orEmpty().ifEmpty { state }
                throw AppFunctionAppUnknownException("print $state: $why")
            }
            PrintResult(
                jobId = job.opt("id").toString(),
                state = state,
                pagesPrinted = job.optInt("pages_done"),
                pagesTotal = job.optInt("pages_total"),
            )
        }

    // ---- helpers ----

    /** Run [block] on an open document and close it afterwards, whatever happens. */
    private suspend fun <T> withDoc(docId: String, block: suspend () -> T): T =
        try {
            block()
        } finally {
            runCatching { app.engine.call("doc.close", "doc_id" to docId) }
        }

    /** Poll job [jobId] until it is done, failed or cancelled. */
    private suspend fun finished(jobId: String): JSONObject {
        while (true) {
            val j = app.engine.call("job.get", "id" to jobId)
            if (j.optString("state") in FINISHED) return j
            delay(POLL_MS)
        }
    }

    /** Save [docId] to the label folder, list it in the app's recents and check it. */
    private suspend fun save(docId: String): SavedLabel {
        val path = app.newDocPath()
        app.engine.call("doc.save", "doc_id" to docId, "path" to path)
        val d = Doc.from(app.engine.call("doc.get", "doc_id" to docId))
        val issues = Issue.list(app.engine.call("doc.check", "doc_id" to docId))
        app.remember(Recent(path, d.name, d.label.widthMm, d.label.heightMm), runCatching { app.engine.render(AppState.THUMB_DPI, docId) }.getOrNull())
        return SavedLabel(
            labelId = File(path).name,
            name = d.name,
            widthMm = d.label.widthMm,
            heightMm = d.label.heightMm,
            warnings = issues.filter { it.severity != "info" }.map { it.message },
        )
    }

    /** The saved label [labelId] names; ids are bare file names in the label folder. */
    private fun labelFile(labelId: String): File {
        val file = File(app.labelsDir, labelId)
        if (labelId.isBlank() || labelId.contains('/') || labelId.startsWith(".") || !file.isFile) {
            throw AppFunctionElementNotFoundException("no label `$labelId`; create one with createLabelFromText or createLabelFromTemplate")
        }
        return file
    }

    /** The elements a template field can set, by field name: text, barcode and QR elements. */
    private fun fieldElements(doc: Doc): List<Pair<String, Element>> =
        doc.elements.mapNotNull { e ->
            val value = if (e.type == "text") e.text else e.data
            val name = e.name ?: value
            if (name.isNullOrBlank() || e.hidden) null else name to e
        }

    private fun Device.toPrinter() = Printer(address, name, connected, model?.name.orEmpty())

    /** Turn engine failures into the AppFunction errors an agent can act on. */
    private suspend fun <T> mapped(block: suspend () -> T): T =
        try {
            block()
        } catch (e: AppFunctionException) {
            throw e
        } catch (e: CommandFailure) {
            val msg = listOfNotNull(e.message?.ifEmpty { null } ?: e.code, e.hint).joinToString("; ")
            throw when (e.code) {
                "bad_params" -> AppFunctionInvalidArgumentException(msg)
                "not_found", "no_document" -> AppFunctionElementNotFoundException(msg)
                "forbidden" -> AppFunctionDeniedException(msg)
                "disabled" -> AppFunctionNotSupportedException(msg)
                "device_unavailable" ->
                    if (msg.contains("permission", ignoreCase = true) || msg.contains("Bluetooth is off")) {
                        AppFunctionPermissionRequiredException("$msg; ask the user to turn Bluetooth on and open Labely once to allow it")
                    } else {
                        AppFunctionAppUnknownException("$msg; ask the user to switch the printer on and bring it close")
                    }
                else -> AppFunctionAppUnknownException(msg)
            }
        }

    private companion object {
        const val DEFAULT_WIDTH_MM = 50.0
        const val DEFAULT_HEIGHT_MM = 30.0
        const val MARGIN_MM = 2.0
        const val TEXT_PT = 28.0
        const val NAME_CHARS = 40
        const val MAX_COPIES = 999
        const val SCAN_MS = 5000
        const val POLL_MS = 300L
        const val PRINT_TIMEOUT_MS = 5 * 60_000L
        val FINISHED = setOf("done", "failed", "cancelled")
    }
}
