package org.labely.app.editor

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.R
import org.labely.app.engine.Element
import org.labely.app.ui.Group
import org.labely.app.ui.IconButton
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * M4 · Adatok és rekordok: the data source, a record stepper and the rows. The canvas above
 * shows the chosen record. Without data it offers the file picker ([onPick]) and the camera
 * scanner ([onScan]), which adds the scanned code as an element.
 */
@Composable
fun DataPanel(
    ed: EditorState,
    modifier: Modifier = Modifier,
    onImages: () -> Unit = {},
    onOptions: () -> Unit = {},
    onScan: () -> Unit = {},
    onPick: () -> Unit,
) {
    val data = ed.data
    if (data == null) {
        Column(modifier.padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically)) {
            Text(stringResource(R.string.data_empty_hint), style = Type.caption, textAlign = TextAlign.Center)
            PrimaryButton(stringResource(R.string.data_choose), onPick, icon = Ic.Open)
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                org.labely.app.ui.SecondaryButton(stringResource(R.string.data_images), onImages, Modifier.weight(1f), Ic.Image)
                org.labely.app.ui.SecondaryButton(stringResource(R.string.serials), onOptions, Modifier.weight(1f), Ic.Hash)
            }
            org.labely.app.ui.SecondaryButton(stringResource(R.string.scan_add), onScan, Modifier.fillMaxWidth(), Ic.Scan)
        }
        return
    }
    val count = data.rowCount
    Column(modifier) {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 10.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            IconTile(Ic.File, Palette.green, Palette.greenTint)
            Column(Modifier.weight(1f).padding(start = 12.dp)) {
                Text(ed.sourceName ?: stringResource(R.string.data_source), style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOf(
                        pluralStringResource(R.plurals.data_records, count, count),
                        pluralStringResource(R.plurals.data_columns, data.columns.size, data.columns.size),
                        stringResource(R.string.data_bound, ed.boundCount),
                    ).joinToString(" · "),
                    style = Type.caption,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            IconButton(Ic.More, onOptions, Palette.text2, size = 36.dp)
            IconButton(Ic.ChevLeft, { ed.showRecord(ed.record - 1) }, Palette.text, size = 36.dp, enabled = ed.record > 1)
            Text(
                stringResource(R.string.record_of, if (count == 0) 0 else ed.record, count),
                style = Type.bodyStrong,
                textAlign = TextAlign.Center,
                maxLines = 1,
                modifier = Modifier.widthIn(min = 44.dp),
            )
            IconButton(Ic.ChevRight, { ed.showRecord(ed.record + 1) }, Palette.text, size = 36.dp, enabled = ed.record < count)
        }
        HorizontalDivider(color = Palette.border)

        val list = rememberLazyListState()
        // Stepping to a record that is loaded but out of sight scrolls to it.
        LaunchedEffect(ed.record) {
            val i = ed.record - 1
            if (i < ed.rows.size && list.layoutInfo.visibleItemsInfo.none { it.index == i }) list.animateScrollToItem(i)
        }
        LazyColumn(Modifier.fillMaxSize(), state = list) {
            itemsIndexed(ed.rows) { i, row ->
                DataRow(i + 1, row, selected = i + 1 == ed.record) { ed.showRecord(i + 1) }
                HorizontalDivider(color = Palette.borderSoft)
            }
            if (ed.rows.size < count) {
                item {
                    // Reaching the end of the loaded rows reads the next page.
                    LaunchedEffect(ed.rows.size) { ed.loadMoreRows() }
                    Text(stringResource(R.string.loading), style = Type.caption, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth().padding(14.dp))
                }
            }
            item {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(
                        stringResource(R.string.data_choose_other),
                        style = Type.bodyStrong.copy(color = Palette.primary),
                        modifier = Modifier.clickable(onClick = onPick).padding(vertical = 10.dp),
                    )
                    Text(
                        stringResource(R.string.data_remove),
                        style = Type.bodyStrong.copy(color = Palette.red),
                        modifier = Modifier.clickable { ed.detachData() }.padding(vertical = 10.dp),
                    )
                }
            }
        }
    }
}

/** One record: its number, the first column, the next two below it. */
@Composable
private fun DataRow(number: Int, row: List<String>, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().background(if (selected) Palette.primaryTint else Palette.surface).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(number.toString(), style = Type.caption.copy(fontWeight = FontWeight.Medium), modifier = Modifier.widthIn(min = 20.dp))
        Column(Modifier.weight(1f)) {
            Text(row.getOrNull(0).orEmpty(), style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
            val rest = row.drop(1).take(2).filter { it.isNotEmpty() }
            if (rest.isNotEmpty()) Text(rest.joinToString(" · "), style = Type.caption, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** The column picker of a text, barcode or 2D code: "None" and the columns of the data source. */
@Composable
fun BindingSheet(ed: EditorState, e: Element, onDismiss: () -> Unit) {
    val bound = ed.boundColumn(e)
    val columns = ed.data?.columns.orEmpty()
    Sheet(stringResource(R.string.binding), onDismiss) {
        Group {
            ListRow(
                stringResource(R.string.binding_none),
                onClick = {
                    ed.unbind(e)
                    onDismiss()
                },
            ) { if (bound == null) LIcon(Ic.Check, Palette.primary, 20.dp) }
            columns.forEach { column ->
                RowDivider()
                ListRow(
                    column,
                    subtitle = ed.value(column)?.ifEmpty { null },
                    onClick = {
                        if (column != bound) ed.bind(e.id, column)
                        onDismiss()
                    },
                    leading = { IconTile(Ic.Link, if (column == bound) Palette.primary else Palette.text2, if (column == bound) Palette.primaryTint else Palette.bg) },
                ) { if (column == bound) LIcon(Ic.Check, Palette.primary, 20.dp) }
            }
        }
        if (columns.isEmpty()) Box(Modifier.fillMaxWidth()) { Text(stringResource(R.string.binding_no_data), style = Type.caption) }
    }
}
