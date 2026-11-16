package org.labely.app

import android.graphics.BitmapFactory
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.labely.ffi.EngineConfig
import org.labely.ffi.LabelyEngine

/**
 * Smoke screen for the Android build (ADR-0026): it loads the Rust engine, renders a label and
 * prints it on the simulator. The real editor arrives with slice 4.3.
 */
class MainActivity : ComponentActivity() {
    private val engine by lazy {
        LabelyEngine(EngineConfig(filesDir.path, emptyList(), allowPrint = true), null)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { MaterialTheme { Surface(Modifier.fillMaxSize()) { SmokeScreen(engine) } } }
    }

    override fun onDestroy() {
        engine.close()
        super.onDestroy()
    }
}

private const val SIM = "sim:niimbot.b21_pro"

private fun LabelyEngine.call(id: String, params: String): JSONObject {
    val env = JSONObject(execute(id, params))
    if (!env.getBoolean("ok")) error(env.getJSONObject("error").getString("message"))
    return env
}

@Composable
private fun SmokeScreen(engine: LabelyEngine) {
    val scope = rememberCoroutineScope()
    var preview by remember { mutableStateOf<ImageBitmap?>(null) }
    var status by remember { mutableStateOf<String?>(null) }
    val done = stringResource(R.string.print_done)
    val failed = stringResource(R.string.print_failed)

    Column(
        Modifier.fillMaxSize().padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(stringResource(R.string.engine_version, engine.apiVersion().toInt()))
        Button(onClick = {
            scope.launch {
                status = withContext(Dispatchers.IO) {
                    runCatching {
                        engine.call("doc.new", """{"width_mm": 40, "height_mm": 12, "printer_model": "niimbot.b21_pro"}""")
                        engine.call(
                            "element.add_text",
                            """{"x_mm": 1, "y_mm": 1, "width_mm": 38, "height_mm": 10, "text": "Labely", "align": "center"}""",
                        )
                        val png = engine.renderPng(null, 203u, 0u)
                        preview = BitmapFactory.decodeByteArray(png, 0, png.size).asImageBitmap()
                        engine.call("device.connect", """{"address": "$SIM"}""")
                        engine.call("print.submit", """{"address": "$SIM"}""")
                    }.fold({ done }, { failed.format(it.message) })
                }
            }
        }) { Text(stringResource(R.string.print_test)) }
        preview?.let { Image(it, contentDescription = null, modifier = Modifier.fillMaxWidth()) }
        status?.let { Text(it) }
    }
}
