package org.labely.app.ui.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import org.labely.app.AppState
import org.labely.app.editor.ConfirmDialog
import org.labely.app.ui.Card
import org.labely.app.ui.IconButton
import org.labely.app.ui.jobs.JobsSheet
import org.labely.app.ui.jobs.jobState
import org.labely.app.ui.theme.LIcon
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * M9 · Előzmények: the print history with each job's printer and state. A tap opens the
 * printed document; the menu prints it again (or continues a job that stopped part-way) and
 * deletes the entry. The title's buttons open the print queue and clear the history.
 */
@Composable
fun HistoryScreen(state: AppState) {
    LaunchedEffect(Unit) { state.refreshHistory() }
    var queue by remember { mutableStateOf(false) }
    var clearing by remember { mutableStateOf(false) }
    val active = state.jobs.count { !it.finished }
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item {
            LargeTitle(stringResource(R.string.tab_history), stringResource(R.string.history_sub)) {
                IconButton(Ic.Printer, { queue = true }, Palette.primary, fill = Palette.primaryTint, size = 44.dp)
                IconButton(Ic.Trash, { clearing = true }, Palette.red, size = 44.dp, enabled = state.history.isNotEmpty())
            }
        }
        if (active > 0 || state.queuePaused) {
            item {
                Card(Modifier.fillMaxWidth(), onClick = { queue = true }) {
                    ListRow(
                        stringResource(R.string.jobs_title),
                        if (state.queuePaused) stringResource(R.string.jobs_paused) else pluralStringResource(R.plurals.jobs_active, active, active),
                        leading = { IconTile(Ic.Printer) },
                    ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
                }
            }
        }
        item {
            Group {
                if (state.history.isEmpty()) ListRow(stringResource(R.string.history_empty))
                state.history.forEachIndexed { i, h ->
                    if (i > 0) RowDivider()
                    val (name, icon, colours) = jobState(h.state)
                    var menu by remember(h.id) { mutableStateOf(false) }
                    val printer = h.model?.let { state.model(it)?.name ?: it } ?: h.address
                    ListRow(
                        title = h.docName,
                        subtitle = listOfNotNull(h.printedAt.take(16), pluralStringResource(R.plurals.labels, h.pages, h.pages), printer.ifEmpty { null }).joinToString(" · "),
                        onClick = if (h.hasFile) ({ state.openHistory(h) }) else null,
                        leading = { IconTile(icon, colours.first, colours.second) },
                    ) {
                        if (h.state != "done") Text(name, style = Type.caption.copy(color = colours.first))
                        Box {
                            IconButton(Ic.More, { menu = true }, size = 36.dp)
                            DropdownMenu(menu, { menu = false }) {
                                if (h.hasFile) {
                                    // A job that stopped part-way is printed again from the print sheet, where the rest is chosen.
                                    DropdownMenuItem(
                                        { Text(stringResource(if (h.state == "done") R.string.history_reprint else R.string.history_continue)) },
                                        { menu = false; state.openHistory(h, print = true) },
                                    )
                                    DropdownMenuItem({ Text(stringResource(R.string.home_open)) }, { menu = false; state.openHistory(h) })
                                }
                                DropdownMenuItem({ Text(stringResource(R.string.delete), color = Palette.red) }, { menu = false; state.deleteHistory(h) })
                            }
                        }
                    }
                    if (h.error != null) Text(h.error, style = Type.caption.copy(color = Palette.red), modifier = Modifier.padding(start = 68.dp, end = 16.dp, bottom = 10.dp))
                }
            }
        }
    }
    if (queue) JobsSheet(state) { queue = false }
    if (clearing) {
        ConfirmDialog(stringResource(R.string.history_clear_title), stringResource(R.string.history_clear_body), stringResource(R.string.history_clear), danger = true, onDismiss = { clearing = false }) {
            state.clearHistory()
            clearing = false
        }
    }
}
