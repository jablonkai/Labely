package org.labely.app.editor

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.R
import org.labely.app.ui.Chip
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.PrimaryButton
import org.labely.app.ui.RowDivider
import org.labely.app.ui.SearchField
import org.labely.app.ui.Stepper
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * The built-in icons (`icon.list`) by category, each drawn by the engine. [onPick] gets the
 * icon's name: to insert it, or to change the selected icon element.
 */
@Composable
fun IconSheet(ed: EditorState, onDismiss: () -> Unit, onPick: (String) -> Unit) {
    val app = ed.app
    LaunchedEffect(Unit) { app.loadIcons() }
    var query by remember { mutableStateOf("") }
    var category by remember { mutableStateOf("") }
    val categories = app.icons.map { it.second }.distinct()
    val shown = app.icons.filter { (name, cat) -> (category.isEmpty() || cat == category) && name.replace('_', ' ').contains(query.trim(), ignoreCase = true) }
    Sheet(stringResource(R.string.kind_icon), onDismiss) {
        SearchField(query, stringResource(R.string.icon_search)) { query = it }
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Chip(stringResource(R.string.filter_all), category.isEmpty()) { category = "" }
            categories.forEach { c -> Chip(c.replace('_', ' ').replaceFirstChar(Char::uppercase), category == c) { category = c } }
        }
        LazyVerticalGrid(GridCells.Fixed(4), Modifier.fillMaxWidth().height(360.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(shown, key = { it.first }) { (name, _) ->
                // Rendered when the tile first shows.
                LaunchedEffect(name) { app.iconThumb(name) }
                Column(
                    Modifier.clip(RoundedCornerShape(12.dp)).background(Palette.bg).clickable {
                        onPick(name)
                        onDismiss()
                    }.padding(6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp)).background(Palette.surface), contentAlignment = Alignment.Center) {
                        app.thumbs["icon:$name"]?.let { Image(it, null, Modifier.fillMaxSize().padding(6.dp), filterQuality = FilterQuality.Medium) }
                    }
                    Text(name.replace('_', ' '), style = Type.small, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
                }
            }
        }
        if (shown.isEmpty()) Text(stringResource(if (app.icons.isEmpty()) R.string.loading else R.string.icon_none), style = Type.caption)
    }
}

/** The eight frame styles of `element.add_border`. */
@Composable
fun BorderSheet(ed: EditorState, onDismiss: () -> Unit) {
    Sheet(stringResource(R.string.kind_border), onDismiss) {
        Group {
            BORDER_STYLES.forEachIndexed { i, style ->
                if (i > 0) RowDivider()
                ListRow(
                    borderStyleName(style),
                    onClick = {
                        ed.addBorder(style)
                        onDismiss()
                    },
                    leading = { IconTile(Ic.Border) },
                ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
            }
        }
    }
}

/** Rows and columns of a new table. */
@Composable
fun NewTableSheet(ed: EditorState, onDismiss: () -> Unit) {
    var rows by remember { mutableIntStateOf(3) }
    var columns by remember { mutableIntStateOf(2) }
    Sheet(stringResource(R.string.kind_table), onDismiss) {
        Group {
            ListRow(stringResource(R.string.table_rows)) { Stepper(rows.toString(), { rows = (rows - 1).coerceAtLeast(1) }, { rows = (rows + 1).coerceAtMost(20) }) }
            RowDivider()
            ListRow(stringResource(R.string.table_columns)) { Stepper(columns.toString(), { columns = (columns - 1).coerceAtLeast(1) }, { columns = (columns + 1).coerceAtMost(12) }) }
        }
        PrimaryButton(
            stringResource(R.string.tool_insert),
            {
                ed.addTable(rows, columns)
                onDismiss()
            },
            Modifier.fillMaxWidth(),
            Ic.Table,
        )
    }
}

/** The codes `code.decode` found in a picked picture; a tap makes an element of one. */
@Composable
fun FoundCodesSheet(ed: EditorState, codes: List<FoundCode>, onDismiss: () -> Unit) {
    Sheet(stringResource(R.string.insert_code_from_image), onDismiss) {
        Text(stringResource(R.string.found_codes_hint), style = Type.caption)
        Group {
            codes.forEachIndexed { i, code ->
                if (i > 0) RowDivider()
                ListRow(
                    code.text,
                    ((if (code.kind == "barcode") SYMBOLOGIES_1D else SYMBOLOGIES_2D).firstOrNull { it.first == code.symbology }?.second ?: code.symbology),
                    onClick = {
                        ed.addCode(code)
                        onDismiss()
                    },
                    leading = { IconTile(if (code.kind == "barcode") Ic.Barcode else Ic.Qr) },
                ) { LIcon(Ic.Plus, Palette.primary, 18.dp) }
            }
        }
    }
}

/** One tile of the insert sheet's grid. */
@Composable
internal fun InsertTile(icon: Ic, label: String, modifier: Modifier, onClick: () -> Unit) {
    Column(
        modifier.aspectRatio(1.2f).clip(RoundedCornerShape(16.dp)).background(Palette.bg).clickable(onClick = onClick).padding(8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically),
    ) {
        LIcon(icon, Palette.primary, 28.dp)
        Text(label, style = Type.bodyStrong.copy(fontSize = 14.sp), maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
    }
}
