package org.labely.app.engine

import org.json.JSONArray
import org.json.JSONObject

/** Typed views of the engine's JSON results. Only what the shell displays is read. */

data class Template(
    val ref: String,
    val name: String,
    val folder: String,
    val starter: Boolean,
    val widthMm: Double,
    val heightMm: Double,
    /** Starter category (`kitchen`, `office`, `cables`, `shipping`, `events`); null for the library. */
    val category: String? = null,
    /** Fits the loaded roll; null when no roll was recognised. */
    val fits: Boolean? = null,
) {
    companion object {
        fun list(result: JSONObject): List<Template> =
            result.optJSONArray("templates").objects().map {
                Template(
                    ref = it.getString("ref"),
                    name = it.optString("name"),
                    folder = it.optString("folder"),
                    starter = it.optBoolean("starter"),
                    widthMm = it.optDouble("width_mm"),
                    heightMm = it.optDouble("height_mm"),
                    category = it.optString("category").takeIf { c -> c.isNotEmpty() },
                    fits = if (it.has("fits")) it.optBoolean("fits") else null,
                )
            }
    }
}

data class Label(
    val widthMm: Double,
    val heightMm: Double,
    val media: String,
    val printerModel: String?,
    val density: Int?,
    val cornerRadiusMm: Double = 0.0,
)

/** A serial counter of the document: `prefix + pad(start + step × n, digits) + suffix`. */
data class Serial(val name: String, val start: Long, val step: Long, val digits: Int, val prefix: String, val suffix: String) {
    /** The value of the label at position [n] (0-based), as the engine computes it. */
    fun value(n: Long): String {
        val v = start + step * n
        val num = if (v < 0) "-" + (-v).toString().padStart(digits, '0') else v.toString().padStart(digits, '0')
        return prefix + num + suffix
    }

    companion object {
        fun map(o: JSONObject?): List<Serial> =
            if (o == null) emptyList() else o.keys().asSequence().toList().sorted().mapNotNull { name ->
                o.optJSONObject(name)?.let {
                    Serial(name, it.optLong("start", 1), it.optLong("step", 1), it.optInt("digits"), it.optString("prefix"), it.optString("suffix"))
                }
            }
    }
}

/** One element of the document, with the raw JSON for kind-specific properties. */
data class Element(
    val id: Long,
    val type: String,
    val name: String?,
    val xMm: Double,
    val yMm: Double,
    val widthMm: Double,
    val heightMm: Double,
    val locked: Boolean,
    val hidden: Boolean,
    val raw: JSONObject,
) {
    val text: String? get() = raw.optString("text").takeIf { raw.has("text") }
    val data: String? get() = raw.optString("data").takeIf { raw.has("data") }
    val fontSizePt: Double get() = raw.optDouble("font_size_pt", if (type == "table") 8.0 else 12.0)
    val group: Long? get() = if (raw.has("group") && !raw.isNull("group")) raw.optLong("group") else null
    val flipH: Boolean get() = raw.optBoolean("flip_h")
    val flipV: Boolean get() = raw.optBoolean("flip_v")

    /** The source element of a linked mirror copy. */
    val mirrorOf: Long? get() = raw.optJSONObject("mirror")?.optLong("of")
    val italic: Boolean get() = raw.optBoolean("italic")
    val underline: Boolean get() = raw.optBoolean("underline")
    val fontFamily: String get() = raw.optString("font_family", "Noto Sans")
    val valign: String get() = raw.optString("valign", "top")
    val fit: String get() = raw.optString("fit", "none")
    val wrap: Boolean get() = raw.optBoolean("wrap", true)
    val lineSpacing: Double get() = raw.optDouble("line_spacing", 1.0)
    val letterSpacingMm: Double get() = raw.optDouble("letter_spacing_mm", 0.0)
    val direction: String get() = raw.optString("direction", "horizontal")
    val maxLengthMm: Double? get() = if (raw.has("max_length_mm") && !raw.isNull("max_length_mm")) raw.optDouble("max_length_mm") else null
    val strokeMm: Double get() = raw.optDouble("stroke_mm", if (type == "border") 0.5 else 0.3)
    val fill: Boolean get() = raw.optBoolean("fill")
    val cornerRadiusMm: Double get() = raw.optDouble("corner_radius_mm", 0.0)
    val bold: Boolean get() = raw.optBoolean("bold")
    val align: String get() = raw.optString("align", "left")
    val rotationDeg: Double get() = raw.optDouble("rotation_deg", 0.0)
    val symbology: String get() = raw.optString("symbology", if (type == "code2d") "qr" else "code128")
    val humanReadable: String get() = raw.optString("human_readable", "below")
    val dither: String get() = raw.optString("dither", "floyd_steinberg")
    val threshold: Int get() = raw.optInt("threshold", 128)
    val invert: Boolean get() = raw.optBoolean("invert")

    /** The string a binding lives in: a text's `text`, a barcode's or 2D code's `data`. */
    val content: String? get() = if (type == "text") text else if (type == "barcode" || type == "code2d") data else null

    fun contains(xMm: Double, yMm: Double): Boolean =
        xMm in this.xMm..(this.xMm + widthMm) && yMm in this.yMm..(this.yMm + heightMm)

    companion object {
        fun from(o: JSONObject) = Element(
            id = o.getLong("id"),
            type = o.optString("type"),
            name = o.optString("name").ifEmpty { null },
            xMm = o.optDouble("x_mm"),
            yMm = o.optDouble("y_mm"),
            widthMm = o.optDouble("width_mm"),
            heightMm = o.optDouble("height_mm"),
            locked = o.optBoolean("locked"),
            hidden = o.optBoolean("hidden"),
            raw = o,
        )
    }
}

/** `doc.get` result. Elements are bottom-first, as drawn. */
data class Doc(
    val docId: String,
    val name: String,
    val path: String?,
    val dirty: Boolean,
    val canUndo: Boolean,
    val canRedo: Boolean,
    val label: Label,
    val elements: List<Element>,
    val variables: Map<String, String> = emptyMap(),
    val serials: List<Serial> = emptyList(),
) {
    companion object {
        fun from(r: JSONObject): Doc {
            val d = r.getJSONObject("document")
            val l = d.getJSONObject("label")
            val page = d.optJSONArray("pages")?.optJSONObject(0)
            return Doc(
                docId = r.getString("doc_id"),
                name = d.optString("name"),
                path = r.optString("path").takeIf { !r.isNull("path") && it.isNotEmpty() },
                dirty = r.optBoolean("dirty"),
                canUndo = r.optBoolean("can_undo"),
                canRedo = r.optBoolean("can_redo"),
                label = Label(
                    widthMm = l.optDouble("width_mm"),
                    heightMm = l.optDouble("height_mm"),
                    media = l.optString("media", "gap"),
                    printerModel = l.optString("printer_model").ifEmpty { null },
                    density = if (l.has("density") && !l.isNull("density")) l.getInt("density") else null,
                    cornerRadiusMm = l.optDouble("corner_radius_mm", 0.0),
                ),
                variables = d.optJSONObject("variables").let { v -> if (v == null) emptyMap() else v.keys().asSequence().associateWith { v.optString(it) } },
                serials = Serial.map(d.optJSONObject("serials")),
                elements = page?.optJSONArray("elements").objects().map(Element::from),
            )
        }
    }
}

data class Model(
    val id: String,
    val name: String,
    val densityMin: Int,
    val densityMax: Int,
    val densityDefault: Int,
    val dpi: Int = 203,
    val headDots: Int = 0,
    val maxWidthMm: Double = 0.0,
    /** Prints 16 gray levels (`grayscale` in print.submit). */
    val gray: Boolean = false,
) {
    /** Width the printhead covers, mm. */
    val headMm: Double get() = if (dpi > 0) headDots * 25.4 / dpi else 0.0
}

/** `device.connect` / `device.list` entry. */
data class Device(
    val address: String,
    val name: String,
    val connected: Boolean,
    val model: Model?,
    val firmware: String?,
    val serial: String?,
) {
    companion object {
        fun from(o: JSONObject) = Device(
            address = o.getString("address"),
            name = o.optString("name"),
            connected = o.optBoolean("connected"),
            model = o.optJSONObject("model")?.let {
                Model(
                    id = it.optString("id"),
                    name = it.optString("name"),
                    densityMin = it.optInt("density_min", 1),
                    densityMax = it.optInt("density_max", 5),
                    densityDefault = it.optInt("density_default", 3),
                    dpi = it.optInt("dpi", 203),
                    headDots = it.optInt("head_dots"),
                    maxWidthMm = it.optDouble("max_width_mm", 0.0),
                    gray = it.optBoolean("gray"),
                )
            },
            firmware = o.optString("firmware").ifEmpty { null },
            serial = o.optString("serial").ifEmpty { null },
        )
    }
}

/** The roll's RFID tag: its barcode and how many labels it had and has used. */
data class RollInfo(val barcode: String, val total: Int, val used: Int, val media: String?) {
    val remaining: Int get() = (total - used).coerceAtLeast(0)
}

data class DeviceStatus(val batteryPercent: Int?, val paperPresent: Boolean?, val lidClosed: Boolean? = null, val roll: RollInfo? = null) {
    companion object {
        fun from(r: JSONObject): DeviceStatus {
            val s = r.optJSONObject("status") ?: JSONObject()
            return DeviceStatus(
                batteryPercent = if (s.isNull("battery_percent")) null else s.optInt("battery_percent"),
                paperPresent = if (s.isNull("paper_present")) null else s.optBoolean("paper_present"),
                lidClosed = if (s.isNull("lid_closed")) null else s.optBoolean("lid_closed"),
                roll = s.optJSONObject("paper")?.let { RollInfo(it.optString("barcode"), it.optInt("total"), it.optInt("used"), it.optString("media").ifEmpty { null }) },
            )
        }
    }
}

/** `device.settings`: only what the printer reports is set. */
data class PrinterSettings(val autoOffLevel: Int?, val soundPower: Boolean?, val soundBluetooth: Boolean?) {
    val empty: Boolean get() = autoOffLevel == null && soundPower == null && soundBluetooth == null

    companion object {
        fun from(r: JSONObject): PrinterSettings {
            val s = r.optJSONObject("settings") ?: JSONObject()
            return PrinterSettings(
                autoOffLevel = if (s.isNull("auto_off_level")) null else s.optInt("auto_off_level"),
                soundPower = if (s.isNull("sound_power")) null else s.optBoolean("sound_power"),
                soundBluetooth = if (s.isNull("sound_bluetooth")) null else s.optBoolean("sound_bluetooth"),
            )
        }
    }
}

/** `data.get` result: the data source and one page of its rows. */
data class DataPage(
    val attached: Boolean,
    val origin: String?,
    val columns: List<String>,
    val rowCount: Int,
    val from: Int,
    val rows: List<List<String>>,
) {
    companion object {
        fun from(r: JSONObject) = DataPage(
            attached = r.optBoolean("attached"),
            origin = r.optString("origin").ifEmpty { null },
            columns = r.optJSONArray("columns").strings(),
            rowCount = r.optInt("row_count"),
            from = r.optInt("from", 1),
            rows = r.optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optJSONArray(it).strings() } },
        )
    }
}

/** `doc.check` issue. */
data class Issue(val rule: String, val severity: String, val message: String, val elementId: Long? = null, val hint: String = "") {
    companion object {
        fun list(r: JSONObject): List<Issue> =
            r.optJSONArray("issues").objects().map {
                Issue(
                    it.optString("rule"),
                    it.optString("severity"),
                    it.optString("message"),
                    if (it.has("element_id") && !it.isNull("element_id")) it.optLong("element_id") else null,
                    it.optString("hint"),
                )
            }
    }
}

data class HistoryEntry(
    val id: String,
    val printedAt: String,
    val docName: String,
    val pages: Int,
    val state: String,
    val address: String = "",
    val model: String? = null,
    val labels: Int = 0,
    val copies: Int = 1,
    val error: String? = null,
    val hasFile: Boolean = true,
) {
    /** Pages the job was to print. */
    val pagesTotal: Int get() = labels * copies
    companion object {
        fun list(r: JSONObject): List<HistoryEntry> =
            r.optJSONArray("entries").objects().map {
                HistoryEntry(
                    id = it.opt("id").toString(),
                    printedAt = it.optString("printed_at"),
                    docName = it.optString("doc_name"),
                    pages = it.optInt("pages_printed"),
                    state = it.optString("state"),
                    address = it.optString("address"),
                    model = it.optString("model").ifEmpty { null },
                    labels = it.optInt("labels"),
                    copies = it.optInt("copies", 1),
                    error = it.optString("error").ifEmpty { null },
                    hasFile = it.has("file") && !it.isNull("file"),
                )
            }
    }
}

/** `job.list` entry, also what the engine pushes on a job change. */
data class JobInfo(
    val id: String,
    val state: String,
    val address: String,
    val docName: String,
    val pagesDone: Int,
    val pagesTotal: Int,
    val submittedAt: String,
    val error: String?,
) {
    val finished: Boolean get() = state == "done" || state == "failed" || state == "cancelled"

    companion object {
        fun from(o: JSONObject) = JobInfo(
            id = o.optString("id"),
            state = o.optString("state"),
            address = o.optString("address"),
            docName = o.optString("doc_name"),
            pagesDone = o.optInt("pages_done"),
            pagesTotal = o.optInt("pages_total"),
            submittedAt = o.optString("submitted_at"),
            error = o.optJSONObject("error")?.optString("message")?.ifEmpty { null },
        )
    }
}

/** `paper.list` entry: a bundled (`std:`) or own (`my:`) paper and the RFID barcodes linked to it. */
data class Paper(
    val id: String,
    val name: String,
    val widthMm: Double,
    val heightMm: Double,
    val cornerRadiusMm: Double,
    val media: String,
    val custom: Boolean,
    val barcodes: List<String>,
) {
    companion object {
        fun from(o: JSONObject) = Paper(
            id = o.optString("id"),
            name = o.optString("name"),
            widthMm = o.optDouble("width_mm"),
            heightMm = o.optDouble("height_mm"),
            cornerRadiusMm = o.optDouble("corner_radius_mm", 0.0),
            media = o.optString("media", "gap"),
            custom = o.optBoolean("custom"),
            barcodes = o.optJSONArray("barcodes").strings(),
        )

        fun list(r: JSONObject): List<Paper> = r.optJSONArray("papers").objects().map(::from)
    }
}

/** `paper.detect` result: what the printer's RFID reader says about the loaded roll. */
data class Detected(
    val rfid: Boolean,
    val barcode: String?,
    val media: String?,
    val remaining: Int?,
    val paper: Paper?,
    val matchesLabel: Boolean?,
) {
    companion object {
        fun from(r: JSONObject) = Detected(
            rfid = r.optBoolean("rfid"),
            barcode = r.optString("barcode").takeIf { !r.isNull("barcode") && it.isNotEmpty() },
            media = r.optString("media").takeIf { !r.isNull("media") && it.isNotEmpty() },
            remaining = if (r.has("remaining") && !r.isNull("remaining")) r.optInt("remaining") else null,
            paper = r.optJSONObject("paper")?.let(Paper::from),
            matchesLabel = if (r.has("matches_label") && !r.isNull("matches_label")) r.optBoolean("matches_label") else null,
        )
    }
}

fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map { optString(it) }

fun JSONArray?.objects(): List<JSONObject> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }
