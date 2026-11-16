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
import org.labely.app.AppState
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** M9 · Előzmények: the print history; a tap reopens the printed document. */
@Composable
fun HistoryScreen(state: AppState) {
    LaunchedEffect(Unit) { state.refreshHistory() }
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item { LargeTitle(stringResource(R.string.tab_history), stringResource(R.string.history_sub)) }
        item {
            Group {
                if (state.history.isEmpty()) ListRow(stringResource(R.string.history_empty))
                state.history.forEachIndexed { i, h ->
                    if (i > 0) RowDivider()
                    val ok = h.state == "done"
                    ListRow(
                        title = h.docName,
                        subtitle = h.printedAt.take(16) + " · " + pluralStringResource(R.plurals.labels, h.pages, h.pages),
                        onClick = { state.openHistory(h) },
                        leading = { IconTile(if (ok) Ic.Printer else Ic.Warn, if (ok) Palette.primary else Palette.amber, if (ok) Palette.primaryTint else Palette.amberTint) },
                    ) {
                        if (!ok) Text(stringResource(R.string.history_failed), style = Type.caption.copy(color = Palette.amber))
                    }
                }
            }
        }
    }
}
