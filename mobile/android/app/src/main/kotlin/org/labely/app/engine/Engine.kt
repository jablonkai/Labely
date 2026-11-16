package org.labely.app.engine

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import java.util.concurrent.Executors
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.labely.ffi.EngineConfig
import org.labely.ffi.LabelyEngine
import org.labely.ffi.NativeBle

/** A command that came back with `ok: false`. [code] is the stable error code. */
class CommandFailure(val code: String, message: String, val hint: String?) : Exception(message)

/**
 * The Rust engine behind a single worker thread (ADR-0027). Every edit is a command, so the
 * shell holds no editing logic: it calls [call] and re-reads the document.
 */
class Engine(dataDir: String, ble: NativeBle?) : AutoCloseable {
    private val raw = LabelyEngine(EngineConfig(dataDir, emptyList(), allowPrint = true), ble)
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "labely-engine") }
    private val dispatcher = worker.asCoroutineDispatcher()

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

    override fun close() {
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
        )
    }
}

/** A JSON object from pairs; `null` values are left out. */
fun json(vararg pairs: Pair<String, Any?>): JSONObject =
    JSONObject().apply { for ((k, v) in pairs) if (v != null) put(k, v) }
