package org.labely.app.editor

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.view.CameraController
import androidx.camera.view.LifecycleCameraController
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import org.labely.app.R
import org.labely.app.ui.IconButton
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Type
import zxingcpp.BarcodeReader
import zxingcpp.BarcodeReader.Format

/** The formats the reader looks for: the ones Labely prints (see [scannedCode]). */
private val SCAN_FORMATS = setOf(
    Format.EAN_13, Format.EAN_8, Format.UPC_A, Format.UPC_E, Format.CODE_128, Format.CODE_39, Format.CODE_93,
    Format.ITF, Format.CODABAR, Format.QR_CODE, Format.DATA_MATRIX, Format.PDF_417, Format.AZTEC,
)

/**
 * The camera scanner (#41): a full-screen preview that reads the first barcode or 2D code it
 * sees with zxing-cpp, on the device, and hands it to [onCode]. It asks for the camera first.
 */
@Composable
fun ScanSheet(onDismiss: () -> Unit, onCode: (FoundCode) -> Unit) {
    val context = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    var denied by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        denied = !it
    }
    LaunchedEffect(Unit) { if (!granted) ask.launch(Manifest.permission.CAMERA) }

    Dialog(onDismiss, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            if (granted) {
                CameraPreview(onCode)
                // The aiming frame.
                Box(Modifier.align(Alignment.Center).size(260.dp, 180.dp).border(3.dp, Color.White, RoundedCornerShape(16.dp)))
            }
            Row(Modifier.fillMaxWidth().statusBarsPadding().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(Ic.Close, onDismiss, Color.White)
                Text(stringResource(R.string.scan_title), style = Type.bodyStrong.copy(color = Color.White), modifier = Modifier.padding(start = 4.dp))
            }
            Column(
                Modifier.align(if (granted) Alignment.BottomCenter else Alignment.Center).fillMaxWidth().navigationBarsPadding().padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                // Nothing while the permission prompt is up.
                if (granted || denied) {
                    Text(
                        stringResource(if (granted) R.string.scan_hint else R.string.scan_no_camera),
                        style = Type.body.copy(color = Color.White),
                        textAlign = TextAlign.Center,
                    )
                }
                if (denied) {
                    PrimaryButton(stringResource(R.string.scan_open_settings), {
                        context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)))
                        onDismiss()
                    })
                }
            }
        }
    }
}

/** The camera preview with zxing-cpp reading its frames; it reports one code, then stops. */
@Composable
private fun CameraPreview(onCode: (FoundCode) -> Unit) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val latest by rememberUpdatedState(onCode)
    val controller = remember { LifecycleCameraController(context).apply { setEnabledUseCases(CameraController.IMAGE_ANALYSIS) } }
    DisposableEffect(owner) {
        val executor = Executors.newSingleThreadExecutor()
        val main = ContextCompat.getMainExecutor(context)
        val reader = BarcodeReader(BarcodeReader.Options(formats = SCAN_FORMATS, tryHarder = true, tryRotate = true))
        val done = AtomicBoolean(false)
        controller.setImageAnalysisAnalyzer(executor) { image ->
            val code = image.use {
                if (done.get()) null else reader.read(it).firstNotNullOfOrNull { r -> scannedCode(r.format.name, r.symbologyIdentifier, r.text) }
            }
            if (code != null && done.compareAndSet(false, true)) main.execute { latest(code) }
        }
        controller.bindToLifecycle(owner)
        onDispose {
            controller.clearImageAnalysisAnalyzer()
            controller.unbind()
            executor.shutdown()
        }
    }
    AndroidView(
        { PreviewView(it).apply { this.controller = controller } },
        Modifier.fillMaxSize(),
    )
}
