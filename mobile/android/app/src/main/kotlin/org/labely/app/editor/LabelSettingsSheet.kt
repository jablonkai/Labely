package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.labely.app.R
import org.labely.app.engine.Issue
import org.labely.app.ui.Banner
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LengthField
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.Segmented
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** On-screen paper colours, as the studio's: white, yellow, pink, blue, green and transparent (grey). */
val LABEL_COLORS = listOf(0xFFFFFFFF, 0xFFFFEC8C, 0xFFFFC4D6, 0xFFB4DCFF, 0xFFC4ECBE, 0xFFE4E4E4).map { it.toInt() }

/**
 * The label's settings: name, paper, size, corner radius, the colour it is shown in, the
 * printer model it is made for with its default darkness, and the canvas aids (grid, snapping,
 * rulers). "Check label" lists the problems `doc.check` finds.
 */
@Composable
fun LabelSettingsSheet(ed: EditorState, onPaper: () -> Unit, onCheck: () -> Unit, onDismiss: () -> Unit) {
    val doc = ed.doc ?: return
    val l = doc.label
    val app = ed.app
    LaunchedEffect(Unit) { app.loadModels() }
    Sheet(stringResource(R.string.label_title), onDismiss) {
        Heading(stringResource(R.string.label_name))
        ContentField(doc.name, stringResource(R.string.new_label_name)) { if (it.isNotBlank()) ed.setLabelProps("name" to it.trim()) }

        Group {
            ListRow(
                stringResource(R.string.paper_library),
                sizeText(l.widthMm, l.heightMm) + " · " + mediaName(l.media),
                onClick = onPaper,
                leading = { IconTile(Ic.Label) },
            ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            LengthField(stringResource(R.string.width), l.widthMm, { if (it > 0) ed.setLabelProps("width_mm" to it) }, Modifier.weight(1f))
            LengthField(stringResource(R.string.height), l.heightMm, { if (it > 0) ed.setLabelProps("height_mm" to it) }, Modifier.weight(1f))
        }
        Group {
            MenuRow(stringResource(R.string.paper_type), mediaName(l.media), MEDIA.map { it to mediaName(it) }) { if (it != l.media) ed.setLabelProps("media" to it) }
            RowDivider()
            ListRow(stringResource(R.string.orientation)) {
                Segmented(listOf(stringResource(R.string.landscape), stringResource(R.string.portrait)), if (l.widthMm >= l.heightMm) 0 else 1) { i ->
                    if ((i == 0) != (l.widthMm >= l.heightMm)) ed.setLabel(l.heightMm, l.widthMm, l.media)
                }
            }
            RowDivider()
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.prop_corner_radius), style = Type.bodyStrong, modifier = Modifier.weight(1f))
                LengthField("", l.cornerRadiusMm, { ed.setLabelProps("corner_radius_mm" to it.coerceAtLeast(0.0)) }, Modifier.weight(1f))
            }
            RowDivider()
            ListRow(stringResource(R.string.label_color), stringResource(R.string.label_color_sub)) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    LABEL_COLORS.forEach { c ->
                        Box(
                            Modifier.size(26.dp).clip(CircleShape).background(Color(c))
                                .border(if (c == app.labelColor) 2.dp else 1.dp, if (c == app.labelColor) Palette.primary else Palette.border, CircleShape)
                                .clickable { app.labelColor = c },
                        )
                    }
                }
            }
        }

        Heading(stringResource(R.string.label_printer))
        Group {
            val model = app.model(l.printerModel)
            val none = stringResource(R.string.model_none)
            val ids = (app.modelIds + listOfNotNull(l.printerModel)).distinct().sorted()
            MenuRow(
                stringResource(R.string.printer_model),
                when {
                    l.printerModel == null -> none
                    model != null -> stringResource(R.string.model_with_dpi, model.name, model.dpi)
                    else -> l.printerModel
                },
                listOf("" to none) + ids.map { id -> id to (app.model(id)?.let { m -> stringResource(R.string.model_with_dpi, m.name, m.dpi) } ?: id) },
            ) { if (it != l.printerModel.orEmpty()) ed.setLabelProps("printer_model" to it) }
            RowDivider()
            val auto = stringResource(R.string.density_model_default)
            val range = (model?.densityMin ?: 1)..(model?.densityMax ?: 5)
            // Density 0 clears the label's own darkness, so the model's default is used.
            MenuRow(stringResource(R.string.density_default), l.density?.toString() ?: auto, listOf("0" to auto) + range.map { it.toString() to it.toString() }) {
                if (it.toInt() != (l.density ?: 0)) ed.setLabelProps("density" to it.toInt())
            }
        }

        Heading(stringResource(R.string.label_canvas))
        CanvasAids(app)

        SecondaryButton(stringResource(R.string.check_label), onCheck, Modifier.fillMaxWidth(), Ic.Check)
    }
}

/** Grid, grid step, snapping and rulers: view settings kept across documents. */
@Composable
fun CanvasAids(app: org.labely.app.AppState) {
    Group {
        SwitchRow(stringResource(R.string.grid_show), stringResource(R.string.grid_show_sub), app.showGrid) { app.showGrid = it }
        RowDivider()
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.grid_step), style = Type.bodyStrong, modifier = Modifier.weight(1f))
            LengthField("", app.gridMm, { if (it > 0) app.gridMm = it }, Modifier.weight(1f))
        }
        RowDivider()
        SwitchRow(stringResource(R.string.snap), stringResource(R.string.snap_sub), app.snap) { app.snap = it }
        RowDivider()
        SwitchRow(stringResource(R.string.rulers), on = app.showRulers) { app.showRulers = it }
    }
}

/** "Check label": every problem `doc.check` finds; a tap selects the element it is about. */
@Composable
fun CheckSheet(ed: EditorState, onDismiss: () -> Unit) {
    LaunchedEffect(Unit) { ed.check() }
    Sheet(stringResource(R.string.check_label), onDismiss) {
        val issues = ed.issues
        when {
            issues == null -> Text(stringResource(R.string.loading), style = Type.caption)
            issues.isEmpty() -> Banner(stringResource(R.string.check_ok), ok = true)
            else -> Group {
                issues.forEachIndexed { i, issue ->
                    if (i > 0) RowDivider()
                    IssueRow(issue) {
                        issue.elementId?.let { ed.selectedId = it }
                        onDismiss()
                    }
                }
            }
        }
        SecondaryButton(stringResource(R.string.check_again), { ed.check() }, Modifier.fillMaxWidth(), Ic.Check)
    }
}

@Composable
private fun IssueRow(issue: Issue, onClick: () -> Unit) {
    val (tint, fill) = when (issue.severity) {
        "error" -> Palette.red to Palette.amberTint
        "warning" -> Palette.amber to Palette.amberTint
        else -> Palette.primary to Palette.primaryTint
    }
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        IconTile(if (issue.severity == "info") Ic.Info else Ic.Warn, tint, fill)
        androidx.compose.foundation.layout.Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(issue.message, style = Type.bodyStrong)
            if (issue.hint.isNotEmpty()) Text(issue.hint, style = Type.caption)
        }
        if (issue.elementId != null) LIcon(Ic.ChevRight, Palette.muted, 18.dp)
    }
}
