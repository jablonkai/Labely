package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.labely.app.KnownPrinter
import org.labely.app.R
import org.labely.app.SIM_ADDRESS
import org.labely.app.ui.Banner
import org.labely.app.ui.Card
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LabelImage
import org.labely.app.ui.ListRow
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.Segmented
import org.labely.app.ui.Stepper
import org.labely.app.ui.printers.PrinterRow
import org.labely.app.ui.printers.rememberBluetoothGate
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** M3 · Nyomtatás: preview, printer, copies, density, the check result and the print button. */
@Composable
fun PrintSheet(ed: EditorState, onDismiss: () -> Unit) {
    val app = ed.app
    val doc = ed.doc ?: return
    val printer = app.printer
    val model = printer?.model
    var copies by remember { mutableIntStateOf(1) }
    var density by remember(model) { mutableIntStateOf(doc.label.density ?: model?.densityDefault ?: 3) }
    var picking by remember { mutableStateOf(false) }
    // With data: the record on the canvas, or one label per record.
    val records = ed.recordCount
    var all by remember { mutableStateOf(false) }
    val labels = copies * if (all && records > 0) records else 1
    LaunchedEffect(printer?.address) { ed.check() }

    Sheet(stringResource(R.string.print), { ed.clearProgress(); onDismiss() }) {
        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(Palette.canvas).padding(24.dp), contentAlignment = Alignment.Center) {
            LabelImage(ed.preview, doc.label.widthMm, doc.label.heightMm, Modifier.fillMaxWidth(0.75f))
        }

        Card(Modifier.fillMaxWidth(), selected = printer != null, onClick = { picking = !picking }) {
            ListRow(
                title = model?.name ?: printer?.name ?: stringResource(R.string.choose_printer),
                subtitle = if (printer != null) {
                    listOfNotNull(
                        app.printerStatus?.batteryPercent?.let { "$it%" },
                        sizeText(doc.label.widthMm, doc.label.heightMm),
                    ).joinToString(" · ")
                } else {
                    stringResource(R.string.no_printer_hint)
                },
                leading = { IconTile(Ic.Printer, if (printer != null) Palette.green else Palette.muted, if (printer != null) Palette.greenTint else Palette.bg) },
            ) { LIcon(Ic.ChevDown, Palette.muted, 18.dp) }
        }
        if (picking || printer == null) PrinterPicker(ed)

        Group {
            if (records > 0) {
                ListRow(stringResource(R.string.records)) {
                    Segmented(listOf(stringResource(R.string.records_current), stringResource(R.string.records_all, records)), if (all) 1 else 0) { all = it == 1 }
                }
                RowDivider()
            }
            ListRow(stringResource(R.string.copies)) {
                Stepper(copies.toString(), { copies = (copies - 1).coerceAtLeast(1) }, { copies = (copies + 1).coerceAtMost(MAX_COPIES) })
            }
            RowDivider()
            val min = model?.densityMin ?: 1
            val max = model?.densityMax ?: 5
            ListRow(stringResource(R.string.density)) {
                Slider(
                    value = density.toFloat(),
                    onValueChange = { density = it.toInt() },
                    valueRange = min.toFloat()..max.toFloat(),
                    steps = (max - min - 1).coerceAtLeast(0),
                    colors = SliderDefaults.colors(thumbColor = Palette.primary, activeTrackColor = Palette.primary, inactiveTrackColor = Palette.primaryTint2),
                    modifier = Modifier.width(140.dp),
                )
                Text("$density / $max", style = Type.bodyStrong)
            }
        }

        CheckBanner(ed, labels)

        val progress = ed.progress
        if (progress != null && !progress.finished) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.printing_progress, progress.done, progress.total), style = Type.bodyStrong)
                LinearProgressIndicator(
                    progress = { if (progress.total > 0) progress.done.toFloat() / progress.total else 0f },
                    modifier = Modifier.fillMaxWidth(),
                    color = Palette.primary,
                )
            }
        } else {
            PrimaryButton(
                pluralStringResource(R.plurals.print_labels, labels, labels),
                { ed.print(copies, density, all && records > 0) },
                Modifier.fillMaxWidth(),
                Ic.Printer,
                enabled = printer != null && app.busy == null && ed.issues?.none { it.severity == "error" } != false,
            )
        }
    }
}

@Composable
private fun CheckBanner(ed: EditorState, copies: Int) {
    val p = ed.progress
    when {
        p?.state == "done" -> Banner(stringResource(R.string.print_done), ok = true, pluralStringResource(R.plurals.labels, p.done, p.done))
        p != null && p.finished -> Banner(stringResource(R.string.print_failed, p.error ?: p.state), ok = false)
        else -> {
            val issues = ed.issues ?: return
            val first = issues.firstOrNull { it.severity == "error" } ?: issues.firstOrNull { it.severity == "warning" }
            if (first == null) {
                Banner(stringResource(R.string.check_ok), ok = true, pluralStringResource(R.plurals.labels, copies, copies))
            } else {
                Banner(first.message, ok = false)
            }
        }
    }
}

/** Known printers and a scan, inside the print sheet. */
@Composable
private fun PrinterPicker(ed: EditorState) {
    val app = ed.app
    val scan = rememberBluetoothGate(app) { app.scan() }
    val known = app.known.filter { it.address != app.printer?.address } +
        if (app.debug && app.known.none { it.address == SIM_ADDRESS } && app.printer?.address != SIM_ADDRESS) {
            listOf(KnownPrinter(SIM_ADDRESS, stringResource(R.string.simulator)))
        } else {
            emptyList()
        }
    val found = app.scanResults.orEmpty().filter { d -> known.none { it.address == d.address } && d.address != app.printer?.address }
    Group {
        (known.map { it.address to it.name } + found.map { it.address to it.name }).forEachIndexed { i, (address, name) ->
            if (i > 0) RowDivider()
            PrinterRow(app, address, name)
        }
        if (known.isNotEmpty() || found.isNotEmpty()) RowDivider()
        Row(Modifier.padding(12.dp)) {
            SecondaryButton(
                stringResource(if (app.busy == org.labely.app.AppState.SCANNING) R.string.scanning else R.string.printers_search),
                scan,
                Modifier.fillMaxWidth(),
                Ic.Bluetooth,
                enabled = app.busy == null,
            )
        }
    }
}

private const val MAX_COPIES = 999
