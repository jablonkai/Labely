package org.labely.app.ui.printers

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.content.Intent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.AppState
import org.labely.app.KnownPrinter
import org.labely.app.R
import org.labely.app.SIM_ADDRESS
import org.labely.app.ble.BlePermissions
import org.labely.app.editor.Sheet
import org.labely.app.ui.Card
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.LSwitch
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.SectionHeader
import org.labely.app.ui.Segmented
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Run [action] once Bluetooth permissions are granted and Bluetooth is on. */
@Composable
fun rememberBluetoothGate(state: AppState, action: () -> Unit): () -> Unit {
    val context = LocalContext.current
    val denied = stringResource(R.string.ble_denied)
    val enable = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        if (context.getSystemService(BluetoothManager::class.java)?.adapter?.isEnabled == true) action()
    }
    fun ensureOn() {
        val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter
        if (adapter != null && !adapter.isEnabled) {
            runCatching { enable.launch(Intent(BluetoothAdapter.ACTION_REQUEST_ENABLE)) }
        } else {
            action()
        }
    }
    val permissions = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
        if (result.values.all { it }) ensureOn() else state.message = denied
    }
    return {
        if (BlePermissions.granted(context)) ensureOn() else permissions.launch(BlePermissions.required)
    }
}

/** M8 · Nyomtatók: the connected printer, known printers and a Bluetooth scan. */
@Composable
fun PrintersScreen(state: AppState) {
    val scan = rememberBluetoothGate(state) { state.scan() }
    LaunchedEffect(state.printer?.address) { state.refreshStatus() }
    val known = state.known.filter { it.address != state.printer?.address }
    val extra = if (state.debug && known.none { it.address == SIM_ADDRESS } && state.printer?.address != SIM_ADDRESS) {
        listOf(KnownPrinter(SIM_ADDRESS, stringResource(R.string.simulator)))
    } else {
        emptyList()
    }
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item {
            LargeTitle(stringResource(R.string.tab_printers), stringResource(R.string.printers_sub)) {
                if (state.busy == AppState.SCANNING) {
                    CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 3.dp)
                } else {
                    IconButton(Ic.Search, scan, Palette.primary, fill = Palette.primaryTint, size = 52.dp)
                }
            }
        }
        item { ConnectedCard(state) }

        if (known.isNotEmpty() || extra.isNotEmpty()) {
            item { SectionHeader(stringResource(R.string.printers_known), stringResource(R.string.printers_search), scan) }
            item {
                Group {
                    (known + extra).forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        PrinterRow(state, k.address, k.name)
                    }
                }
            }
        }

        state.scanResults?.let { all ->
            val found = all.filter { d -> d.address != state.printer?.address && known.none { it.address == d.address } }
            item { SectionHeader(stringResource(R.string.printers_found)) }
            item {
                Group {
                    if (found.isEmpty()) {
                        ListRow(stringResource(R.string.printers_none_found), stringResource(R.string.printers_none_found_hint))
                    }
                    found.forEachIndexed { i, d ->
                        if (i > 0) RowDivider()
                        PrinterRow(state, d.address, d.name)
                    }
                }
            }
        }
    }
}

@Composable
fun PrinterRow(state: AppState, address: String, name: String) {
    val connecting = state.busy == address
    ListRow(
        title = name.ifEmpty { address },
        subtitle = transportName(address) + " · " + address.substringAfter(':'),
        leading = { IconTile(if (address.startsWith("ble:")) Ic.Bluetooth else Ic.Printer) },
    ) {
        if (connecting) {
            CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
        } else {
            val connect = rememberBluetoothGate(state) { state.connect(address, name) }
            Text(
                stringResource(R.string.connect),
                style = Type.bodyStrong.copy(color = Palette.primary),
                modifier = Modifier.clickable(enabled = state.busy == null) {
                    if (address.startsWith("ble:")) connect() else state.connect(address, name)
                },
            )
        }
    }
}

@Composable
private fun transportName(address: String): String = when {
    address.startsWith("ble:") -> stringResource(R.string.transport_ble)
    address.startsWith("usb:") -> "USB"
    else -> stringResource(R.string.transport_sim)
}

@Composable
private fun ConnectedCard(state: AppState) {
    val p = state.printer
    Card(Modifier.fillMaxWidth()) {
        if (p == null) {
            ListRow(
                stringResource(R.string.no_printer),
                stringResource(R.string.no_printer_hint),
                leading = { IconTile(Ic.Printer, Palette.muted, Palette.bg, 56.dp) },
            )
            return@Card
        }
        Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            IconTile(Ic.Printer, size = 64.dp)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(p.model?.name ?: p.name, style = Type.title.copy(fontSize = 19.sp))
                val sub = listOfNotNull(p.serial, p.firmware?.let { stringResource(R.string.firmware, it) }).joinToString(" · ")
                if (sub.isNotEmpty()) Text(sub, style = Type.caption)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Pill(stringResource(R.string.connected), Palette.green, Palette.greenTint)
                    Pill(transportName(p.address), Palette.text2, Palette.field)
                }
            }
        }
        RowDivider()
        val status = state.printerStatus
        Row(Modifier.fillMaxWidth().padding(16.dp)) {
            Stat(stringResource(R.string.battery), status?.batteryPercent?.let { "$it%" } ?: "–", Modifier.weight(1f))
            Stat(
                stringResource(R.string.paper),
                when (status?.paperPresent) {
                    true -> stringResource(R.string.paper_loaded)
                    false -> stringResource(R.string.paper_missing)
                    null -> "–"
                },
                Modifier.weight(1f),
            )
        }
        Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SecondaryButton(stringResource(R.string.calibrate), { state.calibrate() }, Modifier.weight(1f), Ic.Scan, enabled = state.busy == null)
            SecondaryButton(stringResource(R.string.disconnect), { state.disconnect() }, Modifier.weight(1f), Ic.Close, enabled = state.busy == null)
        }
        var settings by remember { mutableStateOf(false) }
        SecondaryButton(
            stringResource(R.string.printer_settings),
            { settings = true },
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
            Ic.Settings,
        )
        if (settings) PrinterSettingsSheet(state) { settings = false }
    }
}

/**
 * The connected printer's own settings: auto power-off, power sound and Bluetooth sound, as far
 * as it reports them (`device.settings`), and calibration.
 */
@Composable
private fun PrinterSettingsSheet(state: AppState, onDismiss: () -> Unit) {
    val p = state.printer
    if (p == null) {
        LaunchedEffect(Unit) { onDismiss() }
        return
    }
    LaunchedEffect(p.address) { state.loadSettings() }
    Sheet(stringResource(R.string.printer_settings), onDismiss) {
        val s = state.printerSettings
        when {
            s == null -> Text(stringResource(R.string.loading), style = Type.caption)
            s.empty -> Text(stringResource(R.string.printer_settings_none), style = Type.caption)
            else -> Group {
                s.autoOffLevel?.let { level ->
                    // NIIMBOT printers switch off after 15, 30, 45 or 60 minutes; others only name the level.
                    val minutes = p.model?.id?.startsWith("niimbot.") == true
                    ListRow(
                        stringResource(R.string.auto_off),
                        if (minutes) stringResource(R.string.auto_off_after, level * 15) else stringResource(R.string.auto_off_level, level),
                    )
                    Segmented(
                        (1..4).map { if (minutes) (it * 15).toString() else it.toString() },
                        level - 1,
                        Modifier.padding(start = 16.dp, end = 16.dp, bottom = 14.dp),
                    ) { if (it + 1 != level) state.setSettings("auto_off_level" to it + 1) }
                }
                s.soundPower?.let { on ->
                    if (s.autoOffLevel != null) RowDivider()
                    ListRow(stringResource(R.string.sound_power), stringResource(R.string.sound_power_sub)) { LSwitch(on) { state.setSettings("sound_power" to it) } }
                }
                s.soundBluetooth?.let { on ->
                    if (s.autoOffLevel != null || s.soundPower != null) RowDivider()
                    ListRow(stringResource(R.string.sound_bluetooth), stringResource(R.string.sound_bluetooth_sub)) { LSwitch(on) { state.setSettings("sound_bluetooth" to it) } }
                }
            }
        }
        SecondaryButton(stringResource(R.string.calibrate), { state.calibrate() }, Modifier.fillMaxWidth(), Ic.Scan, enabled = state.busy == null)
    }
}

@Composable
private fun Pill(text: String, fg: Color, bg: Color) {
    Text(
        text,
        style = Type.caption.copy(color = fg, fontWeight = FontWeight.SemiBold),
        modifier = Modifier.clip(RoundedCornerShape(50)).background(bg).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

@Composable
private fun Stat(label: String, value: String, modifier: Modifier) {
    Column(modifier) {
        Text(label, style = Type.caption)
        Text(value, style = Type.title.copy(fontSize = 20.sp))
    }
}
