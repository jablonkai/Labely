package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.labely.app.AppState
import org.labely.app.R
import org.labely.app.engine.Detected
import org.labely.app.engine.Paper
import org.labely.app.ui.Chip
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.LengthField
import org.labely.app.ui.ListRow
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SearchField
import org.labely.app.ui.SecondaryButton
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * What the printer's RFID reader says about the roll, as the studio's banner: the loaded paper
 * and labels left, a mismatch with the label (with [onUse]) or an unknown roll (with [onLink]).
 * Nothing without a reading or a tag.
 */
@Composable
fun PaperBanner(d: Detected?, modifier: Modifier = Modifier, onUse: (Paper) -> Unit, onLink: () -> Unit) {
    if (d == null || !d.rfid) return
    val paper = d.paper
    val left = d.remaining ?: 0
    val (text, action, ok) = when {
        paper == null -> Triple(stringResource(R.string.paper_unknown, d.barcode.orEmpty(), left), stringResource(R.string.paper_link), false)
        d.matchesLabel == false -> Triple(stringResource(R.string.paper_loaded_name, paper.name, left) + " " + stringResource(R.string.paper_mismatch), stringResource(R.string.paper_use), false)
        else -> Triple(stringResource(R.string.paper_loaded_name, paper.name, left), null, true)
    }
    val fg = if (ok) Palette.green else Palette.amber
    Row(
        modifier.clip(RoundedCornerShape(12.dp)).background(if (ok) Palette.greenTint else Palette.amberTint).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        LIcon(if (ok) Ic.Check else Ic.Warn, fg, 16.dp)
        Text(text, style = Type.caption.copy(color = Palette.text), modifier = Modifier.weight(1f, fill = false))
        if (action != null) {
            Text(action, style = Type.bodyStrong.copy(color = Palette.primary), modifier = Modifier.clickable { if (paper != null) onUse(paper) else onLink() })
        }
    }
}

/**
 * The paper library: bundled and own papers with search, "only fitting" and "mine" filters. A
 * tap uses a paper for the label; the menu of a row links the loaded roll's tag to it, unlinks
 * tags and deletes an own paper. The top reads the loaded roll through the printer's RFID.
 */
@Composable
fun PaperSheet(ed: EditorState, onDismiss: () -> Unit) {
    val app = ed.app
    val label = ed.doc?.label ?: return
    var query by remember { mutableStateOf("") }
    var fitting by remember { mutableStateOf(false) }
    var mine by remember { mutableStateOf(false) }
    var papers by remember { mutableStateOf<List<Paper>>(emptyList()) }
    var version by remember { mutableIntStateOf(0) }
    var adding by remember { mutableStateOf(false) }
    var reading by remember { mutableStateOf(false) }
    val model = label.printerModel
    LaunchedEffect(query, fitting, mine, version) {
        app.report {
            papers = Paper.list(
                app.engine.call("paper.list", "query" to query.trim().ifEmpty { null }, "model" to model.takeIf { fitting }, "custom" to true.takeIf { mine }),
            )
        }
    }
    /** A library change, then the list and the roll's reading again. */
    fun change(id: String, vararg params: Pair<String, Any?>) = app.scope.launch {
        app.report { app.engine.call(id, *params) }
        version++
        if (app.printer != null) app.detectPaper(ed.docId)
    }
    val detected = app.detected
    // The tag of the loaded roll, when it is not linked to a paper yet.
    val loose = detected?.barcode?.takeIf { detected.rfid && detected.paper == null }

    Sheet(stringResource(R.string.paper_library), onDismiss) {
        if (app.printer != null) {
            SecondaryButton(
                stringResource(if (reading) R.string.paper_reading else R.string.paper_read_loaded),
                {
                    reading = true
                    app.scope.launch {
                        ed.detectPaper().join()
                        reading = false
                    }
                },
                Modifier.fillMaxWidth(),
                Ic.Scan,
                enabled = !reading,
            )
            if (detected != null && !detected.rfid) Text(stringResource(R.string.paper_no_tag), style = Type.caption)
            PaperBanner(detected, Modifier.fillMaxWidth(), onUse = { ed.applyPaper(it.id).invokeOnCompletion { app.scope.launch { app.detectPaper(ed.docId) } } }, onLink = {})
            if (loose != null) Text(stringResource(R.string.paper_link_hint), style = Type.caption)
        }
        Group { SwitchRow(stringResource(R.string.paper_auto_apply), stringResource(R.string.paper_auto_apply_sub), app.autoApplyPaper) { app.autoApplyPaper = it } }

        SearchField(query, stringResource(R.string.paper_search)) { query = it }
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (model != null) Chip(stringResource(R.string.paper_only_fitting), fitting) { fitting = !fitting }
            Chip(stringResource(R.string.filter_mine), mine) { mine = !mine }
        }
        Group {
            if (papers.isEmpty()) ListRow(stringResource(R.string.paper_none))
            papers.forEachIndexed { i, p ->
                if (i > 0) RowDivider()
                val current = p.media == label.media && kotlin.math.abs(p.widthMm - label.widthMm) < 0.11 && kotlin.math.abs(p.heightMm - label.heightMm) < 0.11
                var menu by remember(p.id) { mutableStateOf(false) }
                ListRow(
                    p.name,
                    listOfNotNull(sizeText(p.widthMm, p.heightMm), mediaName(p.media), stringResource(R.string.paper_linked, p.barcodes.size).takeIf { p.barcodes.isNotEmpty() }).joinToString(" · "),
                    onClick = {
                        ed.applyPaper(p.id)
                        onDismiss()
                    },
                    leading = { if (current) IconTile(Ic.Label) else IconTile(Ic.Label, Palette.text2, Palette.bg) },
                ) {
                    if (loose != null || p.barcodes.isNotEmpty() || p.custom) {
                        Box {
                            IconButton(Ic.More, { menu = true }, size = 36.dp)
                            DropdownMenu(menu, { menu = false }) {
                                if (loose != null) {
                                    DropdownMenuItem({ Text(stringResource(R.string.paper_link_loaded)) }, { menu = false; change("paper.link", "barcode" to loose, "id" to p.id) })
                                }
                                p.barcodes.forEach { b ->
                                    DropdownMenuItem({ Text(stringResource(R.string.paper_unlink, b)) }, { menu = false; change("paper.unlink", "barcode" to b) })
                                }
                                if (p.custom) {
                                    DropdownMenuItem({ Text(stringResource(R.string.delete), color = Palette.red) }, { menu = false; change("paper.delete", "id" to p.id) })
                                }
                            }
                        }
                    }
                }
            }
        }
        SecondaryButton(stringResource(R.string.paper_add), { adding = true }, Modifier.fillMaxWidth(), Ic.Plus)
    }
    if (adding) {
        AddPaperSheet(app, label.widthMm, label.heightMm, label.cornerRadiusMm, label.media, loose, { adding = false }) { params ->
            adding = false
            change("paper.add", *params)
        }
    }
}

/** A paper of your own: name, size, corner radius and type; optionally linked to the loaded roll's tag. */
@Composable
private fun AddPaperSheet(
    app: AppState,
    widthMm: Double,
    heightMm: Double,
    radiusMm: Double,
    media: String,
    barcode: String?,
    onDismiss: () -> Unit,
    onAdd: (Array<Pair<String, Any?>>) -> Unit,
) {
    var name by remember { mutableStateOf("") }
    var w by remember { mutableDoubleStateOf(widthMm) }
    var h by remember { mutableDoubleStateOf(heightMm) }
    var r by remember { mutableDoubleStateOf(radiusMm) }
    var m by remember { mutableStateOf(media) }
    var link by remember { mutableStateOf(barcode != null) }
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    Sheet(stringResource(R.string.paper_add), onDismiss) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            ContentField(name, stringResource(R.string.paper_name_hint, sizeText(w, h)), live = true) { name = it }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                LengthField(stringResource(R.string.width), w, { if (it > 0) w = it }, Modifier.weight(1f))
                LengthField(stringResource(R.string.height), h, { if (it > 0) h = it }, Modifier.weight(1f))
            }
            LengthField(stringResource(R.string.prop_corner_radius), r, { r = it.coerceAtLeast(0.0) }, Modifier.fillMaxWidth())
            Group {
                MenuRow(stringResource(R.string.paper_type), mediaName(m), MEDIA.map { it to mediaName(it) }) { m = it }
                if (barcode != null) {
                    RowDivider()
                    SwitchRow(stringResource(R.string.paper_link_loaded), barcode, link) { link = it }
                }
            }
        }
        val fallback = sizeText(w, h)
        PrimaryButton(
            stringResource(R.string.paper_add),
            {
                // A size still being typed is committed when its field loses focus.
                focus.clearFocus()
                onAdd(
                    arrayOf(
                        "name" to name.trim().ifEmpty { fallback }, "width_mm" to w, "height_mm" to h, "corner_radius_mm" to r, "media" to m,
                        "barcode" to barcode.takeIf { link },
                    ),
                )
            },
            Modifier.fillMaxWidth(),
            Ic.Plus,
            enabled = app.busy == null,
        )
    }
}
