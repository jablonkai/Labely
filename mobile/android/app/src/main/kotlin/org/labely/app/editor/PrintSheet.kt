package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.ui.platform.LocalFocusManager
import org.labely.app.ui.IconButton
import org.labely.app.ui.LengthField
import org.labely.app.ui.jobs.JobsSheet
import org.labely.app.ui.lengthText
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

/**
 * M3 · Nyomtatás: preview (stepping through the records), printer, which records and how many
 * copies, darkness, the advanced options (offset, mirror, turn 180°), the check result and the
 * print button; a running job shows its progress and can be cancelled.
 */
@Composable
fun PrintSheet(ed: EditorState, onDismiss: () -> Unit) {
    val app = ed.app
    val doc = ed.doc ?: return
    val printer = app.printer
    val model = printer?.model
    // "Continue" from the history: the labels that were left, with the copies of that job.
    val resume = remember { app.resume.also { app.resume = null } }
    var copies by remember { mutableIntStateOf(resume?.third ?: 1) }
    var count by remember { mutableIntStateOf(resume?.let { (it.second - it.first).coerceAtLeast(1) } ?: 1) }
    // "Model default" sends no density: the label's own, else the model's.
    var auto by remember { mutableStateOf(doc.label.density == null) }
    var density by remember(model) { mutableIntStateOf(doc.label.density ?: model?.densityDefault ?: 3) }
    var picking by remember { mutableStateOf(false) }
    var advanced by remember { mutableStateOf(false) }
    var queue by remember { mutableStateOf(false) }
    // With data: the record on the canvas, every record, or a range such as 1,3,5-7.
    val records = ed.recordCount
    var which by remember { mutableIntStateOf(if (resume != null && records > resume.first) 2 else 0) }
    var range by remember { mutableStateOf(if (resume != null && records > resume.first) "${resume.first + 1}-$records" else "") }
    var copiesColumn by remember { mutableStateOf<String?>(null) }
    val saved = remember(printer?.address) { printer?.let { app.printOffset(it.address) } ?: (0.0 to 0.0) }
    var offsetX by remember(printer?.address) { mutableDoubleStateOf(saved.first) }
    var offsetY by remember(printer?.address) { mutableDoubleStateOf(saved.second) }
    var mirror by remember { mutableStateOf(false) }
    var rotate180 by remember { mutableStateOf(false) }
    val picked = if (records > 0 && which == 2) parseRecords(range, records) else null
    val rangeBad = records > 0 && which == 2 && picked == null
    val perCopy = when {
        records == 0 -> count
        which == 1 -> records
        which == 2 -> picked?.size ?: 0
        else -> 1
    }
    val labels = copies * perCopy
    LaunchedEffect(printer?.address) { ed.check() }

    Sheet(stringResource(R.string.print), { ed.clearProgress(); onDismiss() }) {
        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(Palette.canvas).padding(24.dp), contentAlignment = Alignment.Center) {
            LabelImage(ed.preview, doc.label.widthMm, doc.label.heightMm, Modifier.fillMaxWidth(0.75f))
        }
        if (records > 1) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center) {
                IconButton(Ic.ChevLeft, { ed.showRecord(ed.record - 1) }, Palette.text, size = 36.dp, enabled = ed.record > 1)
                Text(stringResource(R.string.record_of, ed.record, records), style = Type.bodyStrong, modifier = Modifier.padding(horizontal = 12.dp))
                IconButton(Ic.ChevRight, { ed.showRecord(ed.record + 1) }, Palette.text, size = 36.dp, enabled = ed.record < records)
            }
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
                    Segmented(listOf(stringResource(R.string.records_current), stringResource(R.string.records_all, records), stringResource(R.string.records_range)), which) { which = it }
                }
                if (which == 2) {
                    Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        ContentField(range, stringResource(R.string.records_range_hint), live = true) { range = it }
                        if (rangeBad && range.isNotBlank()) Text(stringResource(R.string.records_range_bad, records), style = Type.caption.copy(color = Palette.red))
                    }
                }
                RowDivider()
            } else if (doc.serials.isNotEmpty()) {
                // Without data each label advances the serial numbers.
                ListRow(stringResource(R.string.print_count), stringResource(R.string.print_count_sub)) {
                    Stepper(count.toString(), { count = (count - 1).coerceAtLeast(1) }, { count = (count + 1).coerceAtMost(MAX_COUNT) })
                }
                RowDivider()
            }
            ListRow(stringResource(R.string.copies)) {
                Stepper(copies.toString(), { copies = (copies - 1).coerceAtLeast(1) }, { copies = (copies + 1).coerceAtMost(MAX_COPIES) })
            }
            if (records > 0) {
                RowDivider()
                val none = stringResource(R.string.copies_column_none)
                MenuRow(stringResource(R.string.copies_column), copiesColumn ?: none, listOf("" to none) + ed.data?.columns.orEmpty().map { it to it }) { copiesColumn = it.ifEmpty { null } }
            }
            RowDivider()
            val min = model?.densityMin ?: 1
            val max = model?.densityMax ?: 5
            SwitchRow(stringResource(R.string.density), stringResource(R.string.density_model_default), auto) { auto = it }
            if (!auto) {
                ListRow(stringResource(R.string.density_level)) {
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
        }

        Group {
            ListRow(
                stringResource(R.string.print_advanced),
                listOfNotNull(
                    stringResource(R.string.print_offset_value, lengthText(offsetX), lengthText(offsetY)).takeIf { offsetX != 0.0 || offsetY != 0.0 },
                    stringResource(R.string.print_mirror).takeIf { mirror },
                    stringResource(R.string.print_rotate_180).takeIf { rotate180 },
                ).joinToString(" · ").ifEmpty { null },
                onClick = { advanced = !advanced },
            ) { LIcon(if (advanced) Ic.ChevDown else Ic.ChevRight, Palette.muted, 18.dp) }
            if (advanced) {
                Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    LengthField(stringResource(R.string.print_offset_x), offsetX, { offsetX = it.coerceIn(-10.0, 10.0) }, Modifier.weight(1f))
                    LengthField(stringResource(R.string.print_offset_y), offsetY, { offsetY = it.coerceIn(-10.0, 10.0) }, Modifier.weight(1f))
                }
                RowDivider()
                SwitchRow(stringResource(R.string.print_mirror), stringResource(R.string.print_mirror_sub), mirror) { mirror = it }
                RowDivider()
                SwitchRow(stringResource(R.string.print_rotate_180), stringResource(R.string.print_rotate_180_sub), rotate180) { rotate180 = it }
            }
        }

        CheckBanner(ed, labels)

        val progress = ed.progress
        val focus = LocalFocusManager.current
        if (progress != null && !progress.finished) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.printing_progress, progress.done, progress.total), style = Type.bodyStrong)
                LinearProgressIndicator(
                    progress = { if (progress.total > 0) progress.done.toFloat() / progress.total else 0f },
                    modifier = Modifier.fillMaxWidth(),
                    color = Palette.primary,
                )
                SecondaryButton(stringResource(R.string.print_cancel), { ed.cancelPrint() }, Modifier.fillMaxWidth(), Ic.Close)
            }
        } else {
            PrimaryButton(
                if (copiesColumn != null) stringResource(R.string.print) else pluralStringResource(R.plurals.print_labels, labels, labels),
                {
                    // An offset still being typed is committed when its field loses focus.
                    focus.clearFocus()
                    printer?.let { app.setPrintOffset(it.address, offsetX, offsetY) }
                    ed.print(
                        PrintOptions(
                            copies = copies,
                            density = if (auto) null else density,
                            records = when {
                                records == 0 -> null
                                which == 1 -> "all"
                                which == 2 -> recordsParam(range)
                                else -> ed.record.toString()
                            },
                            count = count.takeIf { records == 0 && it > 1 },
                            copiesColumn = copiesColumn,
                            offsetXMm = offsetX,
                            offsetYMm = offsetY,
                            mirror = mirror,
                            rotate180 = rotate180,
                            total = labels,
                        ),
                    )
                },
                Modifier.fillMaxWidth(),
                Ic.Printer,
                enabled = printer != null && app.busy == null && !rangeBad && ed.issues?.none { it.severity == "error" } != false,
            )
        }
        Text(
            stringResource(R.string.jobs_title),
            style = Type.bodyStrong.copy(color = Palette.primary),
            modifier = Modifier.clickable { queue = true }.padding(vertical = 4.dp),
        )
    }
    if (queue) JobsSheet(app) { queue = false }
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
private const val MAX_COUNT = 5000
