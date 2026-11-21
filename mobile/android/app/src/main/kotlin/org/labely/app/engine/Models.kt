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
)

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
    val fontSizePt: Double get() = raw.optDouble("font_size_pt", 10.0)
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
                ),
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
)

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
                )
            },
            firmware = o.optString("firmware").ifEmpty { null },
            serial = o.optString("serial").ifEmpty { null },
        )
    }
}

data class DeviceStatus(val batteryPercent: Int?, val paperPresent: Boolean?) {
    companion object {
        fun from(r: JSONObject): DeviceStatus {
            val s = r.optJSONObject("status") ?: JSONObject()
            return DeviceStatus(
                batteryPercent = if (s.isNull("battery_percent")) null else s.optInt("battery_percent"),
                paperPresent = if (s.isNull("paper_present")) null else s.optBoolean("paper_present"),
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
data class Issue(val rule: String, val severity: String, val message: String) {
    companion object {
        fun list(r: JSONObject): List<Issue> =
            r.optJSONArray("issues").objects().map {
                Issue(it.optString("rule"), it.optString("severity"), it.optString("message"))
            }
    }
}

data class HistoryEntry(
    val id: String,
    val printedAt: String,
    val docName: String,
    val pages: Int,
    val state: String,
) {
    companion object {
        fun list(r: JSONObject): List<HistoryEntry> =
            r.optJSONArray("entries").objects().map {
                HistoryEntry(
                    id = it.opt("id").toString(),
                    printedAt = it.optString("printed_at"),
                    docName = it.optString("doc_name"),
                    pages = it.optInt("pages_printed"),
                    state = it.optString("state"),
                )
            }
    }
}

fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map { optString(it) }

fun JSONArray?.objects(): List<JSONObject> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }
