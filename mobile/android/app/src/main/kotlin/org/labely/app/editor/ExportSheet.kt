package org.labely.app.editor

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import java.io.File
import kotlinx.coroutines.launch
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.ListRow
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.Segmented
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * M12 · Exportálás: PNG or PDF, resolution, and with data the current record or all. The engine
 * writes into its `exports/` folder; Share hands the files to the share sheet, Export to the
 * system's "save to" picker (a folder when there are several files).
 */
@Composable
fun ExportSheet(ed: EditorState, onDismiss: () -> Unit) {
    val app = ed.app
    val context = LocalContext.current
    val appContext = context.applicationContext
    var format by rememberSaveable { mutableStateOf(ExportFormat.Png) }
    var dpi by rememberSaveable { mutableIntStateOf(300) }
    var all by rememberSaveable { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    val count = ed.recordCount
    val exported = stringResource(R.string.export_done)

    /** Copy the files the last export wrote to where the user chose. */
    fun save(write: suspend (List<File>) -> Unit) = app.scope.launch {
        app.report {
            write(app.exportsDir.listFiles().orEmpty().sortedBy { it.name })
            app.message = exported
        }
        onDismiss()
    }
    val savePng = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(ExportFormat.Png.mime)) { uri ->
        if (uri != null) save { files -> saveTo(appContext, files.first(), uri) }
    }
    val savePdf = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(ExportFormat.Pdf.mime)) { uri ->
        if (uri != null) save { files -> saveTo(appContext, files.first(), uri) }
    }
    val saveFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
        if (tree != null) save { files -> saveInto(appContext, files, tree, ExportFormat.Png.mime) }
    }

    /** Export with the chosen options, then hand the files to [then]. */
    fun run(then: (List<File>) -> Unit) = app.scope.launch {
        busy = true
        try {
            app.report {
                val files = ed.export(format, dpi, all && count > 0)
                if (files.isNotEmpty()) then(files)
            }
        } finally {
            busy = false
        }
    }

    Sheet(stringResource(R.string.export), onDismiss) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            FormatTile(Ic.Image, "PNG", format == ExportFormat.Png, Modifier.weight(1f)) { format = ExportFormat.Png }
            FormatTile(Ic.File, "PDF", format == ExportFormat.Pdf, Modifier.weight(1f)) { format = ExportFormat.Pdf }
        }
        Group {
            ListRow(stringResource(R.string.export_resolution)) {
                Segmented(EXPORT_DPI.map { if (it == EXPORT_DPI.last()) stringResource(R.string.export_dpi, it) else it.toString() }, EXPORT_DPI.indexOf(dpi)) { dpi = EXPORT_DPI[it] }
            }
            if (count > 0) {
                RowDivider()
                ListRow(stringResource(R.string.records)) {
                    Segmented(listOf(stringResource(R.string.records_current), stringResource(R.string.records_all, count)), if (all) 1 else 0) { all = it == 1 }
                }
            }
        }
        val names = exportFileNames(ed.doc?.name.orEmpty(), format, if (all && count > 0) listOf(1) else null)
        val files = if (format == ExportFormat.Png && all && count > 0) count else 1
        Text(
            pluralStringResource(R.plurals.export_files, files, files) + " · " + if (files > 1) names[0].replace("-1.png", "-{n}.png") else names[0],
            style = Type.caption,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SecondaryButton(
                stringResource(R.string.export_share),
                { run { files -> shareFiles(context, files, format.mime) } },
                Modifier.weight(1f).height(52.dp),
                Ic.Upload,
                enabled = !busy,
            )
            PrimaryButton(
                stringResource(R.string.export),
                {
                    run { files ->
                        when {
                            files.size > 1 -> saveFolder.launch(null)
                            format == ExportFormat.Pdf -> savePdf.launch(files[0].name)
                            else -> savePng.launch(files[0].name)
                        }
                    }
                },
                Modifier.weight(1f),
                Ic.Download,
                enabled = !busy,
            )
        }
    }
}

@Composable
private fun FormatTile(icon: Ic, label: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Column(
        modifier.clip(shape).background(if (selected) Palette.primaryTint else Palette.surface)
            .border(if (selected) 2.dp else 1.dp, if (selected) Palette.primary else Palette.border, shape)
            .clickable(onClick = onClick).padding(vertical = 16.dp),
        horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        LIcon(icon, if (selected) Palette.primary else Palette.text2, 24.dp)
        Text(label, style = Type.bodyStrong.copy(color = if (selected) Palette.primaryDark else Palette.text))
    }
}
