package org.labely.app.appfunctions

import android.app.Application
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.content.Context
import android.os.Looper
import androidx.appfunctions.AppFunctionData
import androidx.appfunctions.AppFunctionException
import androidx.appfunctions.AppFunctionInvalidArgumentException
import androidx.appfunctions.AppFunctionPermissionRequiredException
import androidx.appfunctions.AppFunctionSearchSpec
import androidx.appfunctions.ExecuteAppFunctionRequest
import androidx.appfunctions.ExecuteAppFunctionResponse
import androidx.appfunctions.metadata.AppFunctionArrayTypeMetadata
import androidx.appfunctions.metadata.AppFunctionMetadata
import androidx.appfunctions.metadata.AppFunctionObjectTypeMetadata
import androidx.appfunctions.metadata.AppFunctionReferenceTypeMetadata
import androidx.appfunctions.testing.AppFunctionTestRule
import androidx.test.core.app.ApplicationProvider
import java.io.File
import java.time.Duration
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.async
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.labely.app.LabelyApplication
import org.labely.app.SIM_ADDRESS
import org.labely.app.ble.BlePermissions
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * The AppFunctions through `appfunctions-testing`, as an agent calls them, on the real engine
 * (built for the host and loaded through JNA) and the B21 Pro simulator (ADR-0028).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class LabelyFunctionsTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @get:Rule val rule = AppFunctionTestRule(context)

    private val app get() = (context as LabelyApplication).state

    @Test
    fun registersTheSixFunctions() {
        val names = onMain { functions() }.map { it.id.substringAfter('#') }.toSet()
        assertEquals(
            setOf("createLabelFromText", "createLabelFromTemplate", "listTemplates", "listPrinters", "getPrinterStatus", "printLabel"),
            names,
        )
    }

    @Test
    fun listTemplatesGivesStartersWithTheirFields() {
        val templates = result(call("listTemplates")).list(LabelTemplate::class.java)
        val price = templates.single { it.templateId == "starter:price-tag" }
        assertEquals(listOf("text", "text", "barcode"), price.fields.map { it.kind })
        assertTrue(price.fields.any { it.name == "Product name" })
        assertTrue(templates.size >= 9)
    }

    @Test
    fun createLabelFromTextSavesALabelTheAppLists() {
        val label = result(call("createLabelFromText") { setString("text", "Spices\nCumin") }).single(SavedLabel::class.java)
        assertEquals(50.0, label.widthMm, 0.001)
        assertEquals("Spices", label.name)
        assertEquals(emptyList<String>(), label.warnings)
        val file = File(app.labelsDir, label.labelId)
        assertTrue(file.isFile)
        assertTrue(file.readText().contains("Cumin"))
        assertEquals(file.path, app.recents.first().path)
    }

    @Test
    fun createLabelFromTextRejectsEmptyTextAndBadSizes() {
        failure<AppFunctionInvalidArgumentException>(call("createLabelFromText") { setString("text", "  ") })
        failure<AppFunctionInvalidArgumentException>(
            call("createLabelFromText") { setString("text", "x"); setDouble("widthMm", 500.0) },
        )
    }

    @Test
    fun createLabelFromTemplateFillsFields() {
        val label = result(
            call("createLabelFromTemplate") { meta ->
                setString("templateId", "starter:price-tag")
                setAppFunctionDataList("fields", listOf(field(meta, "Product name", "Apples"), field(meta, "9.99", "2.49")))
            },
        ).single(SavedLabel::class.java)
        val json = File(app.labelsDir, label.labelId).readText()
        assertTrue(json.contains("Apples") && json.contains("2.49"))
        assertTrue(!json.contains("Product name"))
    }

    @Test
    fun createLabelFromTemplateNamesTheFieldsOnAWrongField() {
        val e = failure<AppFunctionInvalidArgumentException>(
            call("createLabelFromTemplate") { meta ->
                setString("templateId", "starter:price-tag")
                setAppFunctionDataList("fields", listOf(field(meta, "Price", "1")))
            },
        )
        assertTrue(e.errorMessage.orEmpty().contains("`Product name`"))
    }

    @Test
    fun printLabelNeedsTheUsersConsent() {
        val label = result(call("createLabelFromText") { setString("text", "Hi") }).single(SavedLabel::class.java)
        failure<AppFunctionPermissionRequiredException>(
            call("printLabel") { setString("labelId", label.labelId); setString("printerId", SIM_ADDRESS) },
        )
    }

    @Test
    fun printLabelPrintsOnTheSimulator() {
        val label = result(call("createLabelFromText") { setString("text", "Hi") }).single(SavedLabel::class.java)
        app.allowAgentPrint = true
        val printed = result(
            call("printLabel") { setString("labelId", label.labelId); setString("printerId", SIM_ADDRESS); setInt("copies", 2) },
        ).single(PrintResult::class.java)
        assertEquals("done", printed.state)
        assertEquals(2, printed.pagesPrinted)
        assertEquals(1, app.history.size)

        val printers = result(call("listPrinters")).list(Printer::class.java)
        assertTrue(printers.first().let { it.printerId == SIM_ADDRESS && it.connected })
        val status = result(call("getPrinterStatus") { setString("printerId", SIM_ADDRESS) }).single(PrinterStatus::class.java)
        assertTrue(status.batteryPercent in -1..100)
    }

    @Test
    fun printLabelRejectsAnUnknownLabel() {
        app.allowAgentPrint = true
        failure<androidx.appfunctions.AppFunctionElementNotFoundException>(
            call("printLabel") { setString("labelId", "../secret.labely"); setString("printerId", SIM_ADDRESS) },
        )
    }

    @Test
    fun listPrintersScanAsksForBluetoothPermission() {
        val e = failure<AppFunctionPermissionRequiredException>(call("listPrinters") { setBoolean("scan", true) })
        assertTrue(e.errorMessage.orEmpty().contains("Bluetooth"))
    }

    @Test
    fun listPrintersScanSearchesWhenAllowed() {
        shadowOf(context as Application).grantPermissions(*BlePermissions.required)
        val adapter = context.getSystemService(BluetoothManager::class.java).adapter
        shadowOf(adapter).setState(BluetoothAdapter.STATE_ON)
        shadowOf(adapter).setEnabled(true)
        val printers = result(call("listPrinters") { setBoolean("scan", true) }).list(Printer::class.java)
        // Robolectric's scanner finds nothing, so only the debug simulator is listed.
        assertEquals(listOf(SIM_ADDRESS), printers.map { it.printerId })
    }

    // ---- helpers ----

    private suspend fun functions(): List<AppFunctionMetadata> =
        rule.getAppFunctionManager().searchAppFunctions(AppFunctionSearchSpec(packageNames = setOf(context.packageName)))

    /** Call [name] as an agent would, with parameters set by [params]. */
    private fun call(name: String, params: AppFunctionData.Builder.(AppFunctionMetadata) -> Unit = {}): ExecuteAppFunctionResponse =
        onMain {
            val meta = functions().single { it.id.endsWith("#$name") }
            val data = AppFunctionData.Builder(meta.parameters, meta.components).apply { params(meta) }.build()
            rule.getAppFunctionManager().executeAppFunction(ExecuteAppFunctionRequest(context.packageName, meta.id, data))
        }

    /** A `FieldValue` for the `fields` parameter of [meta]. */
    private fun field(meta: AppFunctionMetadata, name: String, value: String): AppFunctionData {
        val item = (meta.parameters.single { it.name == "fields" }.dataType as AppFunctionArrayTypeMetadata).itemType
        val type = if (item is AppFunctionReferenceTypeMetadata) meta.components.dataTypes.getValue(item.referenceDataType) else item
        return AppFunctionData.serialize(type as AppFunctionObjectTypeMetadata, meta.components, FieldValue(name, value))
    }

    private fun result(r: ExecuteAppFunctionResponse): Returned {
        if (r is ExecuteAppFunctionResponse.Error) throw AssertionError("call failed: ${r.error.errorMessage}", r.error)
        return Returned((r as ExecuteAppFunctionResponse.Success).returnValue)
    }

    private class Returned(val data: AppFunctionData) {
        fun <T : Any> single(c: Class<T>): T =
            data.getAppFunctionData(ExecuteAppFunctionResponse.Success.PROPERTY_RETURN_VALUE)!!.deserialize(c)

        fun <T : Any> list(c: Class<T>): List<T> =
            data.getAppFunctionDataList(ExecuteAppFunctionResponse.Success.PROPERTY_RETURN_VALUE).orEmpty().map { it.deserialize(c) }
    }

    private inline fun <reified E : AppFunctionException> failure(r: ExecuteAppFunctionResponse): E {
        val error = (r as? ExecuteAppFunctionResponse.Error)?.error ?: throw AssertionError("expected ${E::class.simpleName}, got success")
        if (error !is E) throw AssertionError("expected ${E::class.simpleName}, got ${error::class.simpleName}: ${error.errorMessage}", error)
        return error
    }

    /**
     * Run [block] on the main thread, where the app's state and the AppFunction service live,
     * and run the main looper until it is done. Engine calls finish on the engine's thread.
     */
    @OptIn(DelicateCoroutinesApi::class)
    private fun <T> onMain(block: suspend () -> T): T {
        val job = GlobalScope.async(Dispatchers.Main) { block() }
        val looper = shadowOf(Looper.getMainLooper())
        val deadline = System.nanoTime() + 60_000_000_000L
        while (!job.isCompleted) {
            if (System.nanoTime() > deadline) throw AssertionError("call did not finish in 60 s")
            // Advance the paused clock too, so delays (job polling) come due.
            looper.idleFor(Duration.ofMillis(10))
            Thread.sleep(1)
        }
        job.getCompletionExceptionOrNull()?.let { throw it }
        @OptIn(ExperimentalCoroutinesApi::class)
        return job.getCompleted()
    }
}
