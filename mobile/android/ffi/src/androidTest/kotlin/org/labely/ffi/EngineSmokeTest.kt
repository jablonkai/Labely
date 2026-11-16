package org.labely.ffi

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The Rust engine loads on a device and runs commands end to end (ADR-0026). */
@RunWith(AndroidJUnit4::class)
class EngineSmokeTest {
    private val dataDir = InstrumentationRegistry.getInstrumentation().targetContext.filesDir

    private fun engine(allowPrint: Boolean = true) =
        LabelyEngine(EngineConfig(dataDir.path, emptyList(), allowPrint), null)

    private fun LabelyEngine.run(id: String, params: String = "{}"): JSONObject {
        val env = JSONObject(execute(id, params))
        assertTrue("$id: $env", env.getBoolean("ok"))
        return env.optJSONObject("result") ?: JSONObject()
    }

    @Test
    fun rendersAndPrintsOnTheSimulator() {
        engine().use { e ->
            assertEquals(3u, e.apiVersion())
            e.run("doc.new", """{"width_mm": 40, "height_mm": 12, "printer_model": "niimbot.b21_pro"}""")
            e.run(
                "element.add_text",
                """{"x_mm": 1, "y_mm": 1, "width_mm": 38, "height_mm": 10, "text": "Labely"}""",
            )
            val png = e.renderPng(null, 203u, 0u)
            assertEquals("PNG", String(png, 1, 3))

            e.run("device.connect", """{"address": "sim:niimbot.b21_pro"}""")
            val job = e.run("print.submit", """{"address": "sim:niimbot.b21_pro"}""")
            assertEquals("done", job.getString("state"))
        }
    }

    @Test
    fun errorsAndPolicyComeBackAsEnvelopes() {
        engine(allowPrint = false).use { e ->
            val bad = JSONObject(e.execute("doc.new", "[1]"))
            assertFalse(bad.getBoolean("ok"))
            assertEquals("bad_params", bad.getJSONObject("error").getString("code"))

            e.run("doc.new", """{"width_mm": 20, "height_mm": 8}""")
            val outside = JSONObject(e.execute("doc.save", """{"path": "/sdcard/x.labely"}"""))
            assertEquals("forbidden", outside.getJSONObject("error").getString("code"))
            e.run("doc.save", JSONObject().put("path", "${dataDir.path}/a.labely").toString())

            e.run("device.connect", """{"address": "sim:niimbot.b21_pro"}""")
            val print = JSONObject(e.execute("print.submit", """{"address": "sim:niimbot.b21_pro"}"""))
            assertEquals("forbidden", print.getJSONObject("error").getString("code"))
            e.setAllowPrint(true)
            e.run("print.submit", """{"address": "sim:niimbot.b21_pro"}""")

            try {
                e.renderPng("no-such-doc", 203u, 0u)
                throw AssertionError("expected EngineException")
            } catch (_: EngineException) {
            }
        }
    }
}
