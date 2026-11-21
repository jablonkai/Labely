package org.labely.app.engine

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import java.util.Base64
import java.util.concurrent.Executors
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.labely.ffi.DeviceEvents
import org.labely.ffi.EngineConfig
import org.labely.ffi.LabelyEngine
import org.labely.ffi.NativeBle

/** A command that came back with `ok: false`. [code] is the stable error code. */
class CommandFailure(val code: String, message: String, val hint: String?, val accepted: List<String> = emptyList()) : Exception(message)

/**
 * The Rust engine behind a single worker thread (ADR-0027). Every edit is a command, so the
 * shell holds no editing logic: it calls [call] and re-reads the document.
 *
 * The engine pushes job and printer changes (ADR-0031): [job] follows one job and [devices]
 * carries the printer events, so nothing polls.
 */
class Engine(dataDir: String, ble: NativeBle?) : AutoCloseable {
    private val raw = LabelyEngine(EngineConfig(dataDir, emptyList(), allowPrint = true), ble)
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "labely-engine") }
    private val dispatcher = worker.asCoroutineDispatcher()

    /** The latest state of each job this session, by job id (the `job.get` result). */
    private val jobs = MutableStateFlow<Map<String, JSONObject>>(emptyMap())
    private val deviceEvents = MutableSharedFlow<JSONObject>(extraBufferCapacity = 64, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /**
     * Printer events: `{"address", "connected", "device"?, "status"?, "lost"?}` after
     * `device.connect`, `device.disconnect`, `device.status`, or when a link drops.
     */
    val devices: SharedFlow<JSONObject> = deviceEvents.asSharedFlow()

    init {
        // Called on the engine's event thread, in order. Must not throw.
        raw.setEvents(object : DeviceEvents {
            override fun jobChanged(jobJson: String) {
                runCatching {
                    val j = JSONObject(jobJson)
                    val id = j.getString("id")
                    jobs.update { m ->
                        LinkedHashMap(m).apply {
                            remove(id)
                            put(id, j)
                            while (size > MAX_JOBS) remove(keys.first())
                        }
                    }
                }
            }

            override fun deviceChanged(eventJson: String) {
                runCatching { deviceEvents.tryEmit(JSONObject(eventJson)) }
            }
        })
    }

    /** Every job of this session by id, oldest first; changes as the engine pushes them. */
    val allJobs: StateFlow<Map<String, JSONObject>> = jobs.asStateFlow()

    /** Every change of job [id], starting with the latest one already seen. */
    fun job(id: String): Flow<JSONObject> = jobs.mapNotNull { it[id] }.distinctUntilChanged { a, b -> a.toString() == b.toString() }

    /** Suspend until job [id] is done, failed or cancelled; returns its last state. */
    suspend fun finished(id: String): JSONObject = job(id).first { it.optString("state") in FINISHED }

    val apiVersion: Int get() = raw.apiVersion().toInt()

    /** Run [id] and return its `result`, or throw [CommandFailure]. */
    suspend fun call(id: String, params: JSONObject = JSONObject()): JSONObject =
        withContext(dispatcher) { unwrap(raw.execute(id, params.toString())) }

    /** Shorthand for params given as `key to value` pairs. */
    suspend fun call(id: String, vararg params: Pair<String, Any?>): JSONObject =
        call(id, json(*params))

    /** The exact 1-bit print raster of page 0 of the active (or [docId]) document. */
    suspend fun render(dpi: Int, docId: String? = null): ImageBitmap =
        withContext(dispatcher) {
            val png = raw.renderPng(docId, dpi.toUInt(), 0u)
            BitmapFactory.decodeByteArray(png, 0, png.size).asImageBitmap()
        }

    /** The print raster of the document with data record [record] (1-based) filled in. */
    suspend fun preview(dpi: Int, docId: String, record: Int): ImageBitmap {
        val b64 = call("render.preview", "doc_id" to docId, "dpi" to dpi, "record" to record).getString("png_base64")
        return withContext(dispatcher) {
            val png = Base64.getDecoder().decode(b64)
            BitmapFactory.decodeByteArray(png, 0, png.size).asImageBitmap()
        }
    }

    /** Several document commands as one undo step (`command.batch`, all or nothing). */
    suspend fun batch(docId: String, steps: List<Pair<String, JSONObject>>, label: String? = null): JSONObject =
        call(
            "command.batch",
            json(
                "steps" to org.json.JSONArray(steps.map { (id, params) -> JSONObject().put("id", id).put("params", params.put("doc_id", docId)) }),
                "label" to label,
                "terse" to true,
            ),
        )

    override fun close() {
        raw.setEvents(null)
        worker.shutdown()
        raw.close()
    }

    private fun unwrap(envelope: String): JSONObject {
        val env = JSONObject(envelope)
        if (env.optBoolean("ok")) return env.optJSONObject("result") ?: JSONObject()
        val err = env.optJSONObject("error") ?: JSONObject()
        throw CommandFailure(
            err.optString("code", "internal"),
            err.optString("message"),
            err.optString("hint").ifEmpty { null },
            err.optJSONArray("accepted").let { a -> if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) } },
        )
    }

    companion object {
        val FINISHED = setOf("done", "failed", "cancelled")
        private const val MAX_JOBS = 100
    }
}

/** A JSON object from pairs; `null` values are left out. */
fun json(vararg pairs: Pair<String, Any?>): JSONObject =
    JSONObject().apply { for ((k, v) in pairs) if (v != null) put(k, v) }
