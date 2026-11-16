package org.labely.app.ui.templates

import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.AppState
import org.labely.app.R
import org.labely.app.engine.Template
import org.labely.app.ui.Card
import org.labely.app.ui.Chip
import org.labely.app.ui.IconButton
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.SearchField
import org.labely.app.ui.ThumbTile
import org.labely.app.ui.sizeText
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

private const val ALL = ""
private const val STARTERS = "\u0000starters"
private const val MINE = "\u0000mine"

/** M7 · Sablonkönyvtár: starter templates and the user's library, by folder. */
@Composable
fun TemplatesScreen(state: AppState, onNewLabel: () -> Unit) {
    var query by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf(ALL) }
    var deleting by remember { mutableStateOf<Template?>(null) }
    val context = LocalContext.current
    val folders = state.templates.filter { !it.starter && it.folder.isNotEmpty() }.map { it.folder }.distinct().sorted()
    val shown = state.templates.filter { t ->
        val inFilter = when (filter) {
            ALL -> true
            STARTERS -> t.starter
            MINE -> !t.starter
            else -> !t.starter && t.folder == filter
        }
        inFilter && (query.isEmpty() || t.displayName(context).contains(query, ignoreCase = true))
    }
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item {
            LargeTitle(stringResource(R.string.tab_templates), stringResource(R.string.templates_sub)) {
                IconButton(Ic.Plus, onNewLabel, Color.White, fill = Palette.primary, size = 52.dp)
            }
        }
        item { SearchField(query, stringResource(R.string.templates_search)) { query = it } }
        item {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip(stringResource(R.string.filter_all), filter == ALL) { filter = ALL }
                Chip(stringResource(R.string.filter_starters), filter == STARTERS) { filter = STARTERS }
                Chip(stringResource(R.string.filter_mine), filter == MINE) { filter = MINE }
                folders.forEach { f -> Chip(f, filter == f) { filter = f } }
            }
        }
        item { Text(pluralStringResource(R.plurals.templates_count, shown.size, shown.size), style = Type.bodyStrong.copy(color = Palette.muted)) }
        item { TemplateGrid(shown, state, onOpen = state::fromTemplate, onLongPress = { if (!it.starter) deleting = it }) }
        item { Spacer(Modifier.padding(4.dp)) }
    }
    deleting?.let { t ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.template_delete_title)) },
            text = { Text(stringResource(R.string.template_delete_body, templateName(t))) },
            confirmButton = {
                TextButton({ state.deleteTemplate(t); deleting = null }) { Text(stringResource(R.string.delete), color = Palette.red) }
            },
            dismissButton = { TextButton({ deleting = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Two-column grid of template cards; laid out in rows so it can sit inside a lazy list. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun TemplateGrid(templates: List<Template>, state: AppState, onOpen: (Template) -> Unit, onLongPress: (Template) -> Unit = {}) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        templates.chunked(2).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { t ->
                    Card(Modifier.weight(1f)) {
                        Column(
                            Modifier.clip(RoundedCornerShape(18.dp)).combinedClickable(onClick = { onOpen(t) }, onLongClick = { onLongPress(t) }).padding(10.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            ThumbTile(state.thumbs[t.ref], t.widthMm, t.heightMm)
                            Column(Modifier.padding(horizontal = 4.dp)) {
                                Text(templateName(t), style = Type.bodyStrong.copy(fontSize = 16.sp), maxLines = 1)
                                Text(sizeText(t.widthMm, t.heightMm), style = Type.caption)
                            }
                        }
                    }
                }
                if (row.size == 1) Box(Modifier.weight(1f))
            }
        }
    }
}

private val STARTER_NAMES = mapOf(
    "starter:name-badge" to R.string.template_name_badge,
    "starter:price-tag" to R.string.template_price_tag,
    "starter:asset-tag" to R.string.template_asset_tag,
    "starter:address" to R.string.template_address,
    "starter:qr-link" to R.string.template_qr_link,
    "starter:storage-box" to R.string.template_storage_box,
    "starter:jar" to R.string.template_jar,
    "starter:cable-flag" to R.string.template_cable_flag,
    "starter:wifi" to R.string.template_wifi,
    "starter:shipping" to R.string.template_shipping,
)

/** Starter names are translated like in the studio; library templates keep their own name. */
fun Template.displayName(context: Context): String = STARTER_NAMES[ref]?.let(context::getString) ?: name

@Composable
fun templateName(t: Template): String = t.displayName(LocalContext.current)
