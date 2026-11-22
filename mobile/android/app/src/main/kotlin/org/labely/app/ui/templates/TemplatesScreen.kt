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
import org.labely.app.editor.ConfirmDialog
import org.labely.app.editor.Sheet
import org.labely.app.editor.TextDialog
import org.labely.app.engine.Template
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
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

/**
 * M7 · Sablonkönyvtár: starter templates and the user's library, by folder. Folders are made,
 * renamed and deleted here; a long press on a template duplicates, renames, moves or deletes it.
 */
@Composable
fun TemplatesScreen(state: AppState, onNewLabel: () -> Unit) {
    var query by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf(ALL) }
    var category by remember { mutableStateOf<String?>(null) }
    var deleting by remember { mutableStateOf<Template?>(null) }
    var acting by remember { mutableStateOf<Template?>(null) }
    var renaming by remember { mutableStateOf<Template?>(null) }
    var moving by remember { mutableStateOf<Template?>(null) }
    var newFolder by remember { mutableStateOf(false) }
    var renameFolder by remember { mutableStateOf<String?>(null) }
    var deleteFolder by remember { mutableStateOf<String?>(null) }
    val context = LocalContext.current
    val folders = (state.folders + state.templates.filter { !it.starter && it.folder.isNotEmpty() }.map { it.folder }).distinct().sorted()
    // A folder that was renamed or deleted is no longer a filter.
    androidx.compose.runtime.LaunchedEffect(folders) { if (filter !in listOf(ALL, STARTERS, MINE) && filter !in folders) filter = MINE }
    val shown = state.templates.filter { t ->
        val inFilter = when (filter) {
            ALL -> true
            STARTERS -> t.starter && (category == null || t.category == category)
            MINE -> !t.starter
            else -> !t.starter && t.folder == filter
        }
        inFilter && (query.isEmpty() || t.displayName(context).contains(query, ignoreCase = true))
    }
    val isFolder = filter in folders
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
                Chip("+ " + stringResource(R.string.folder), false) { newFolder = true }
            }
        }
        if (filter == STARTERS) {
            item {
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Chip(stringResource(R.string.filter_all), category == null) { category = null }
                    CATEGORIES.forEach { (id, res) -> Chip(stringResource(res), category == id) { category = id } }
                }
            }
        }
        item {
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Text(pluralStringResource(R.plurals.templates_count, shown.size, shown.size), style = Type.bodyStrong.copy(color = Palette.muted), modifier = Modifier.weight(1f))
                if (isFolder) {
                    IconButton(Ic.Edit, { renameFolder = filter }, size = 36.dp)
                    IconButton(Ic.Trash, { deleteFolder = filter }, Palette.red, size = 36.dp)
                }
            }
        }
        item { TemplateGrid(shown, state, onOpen = state::fromTemplate, onLongPress = { acting = it }) }
        item { Text(stringResource(R.string.templates_hint), style = Type.caption) }
        item { Spacer(Modifier.padding(4.dp)) }
    }

    acting?.let { t ->
        Sheet(templateName(t), { acting = null }) {
            Group {
                ListRow(stringResource(if (t.starter) R.string.template_copy_to_mine else R.string.duplicate), onClick = { state.duplicateTemplate(t); acting = null }, leading = { IconTile(Ic.Copy) })
                if (!t.starter) {
                    RowDivider()
                    ListRow(stringResource(R.string.rename), onClick = { renaming = t; acting = null }, leading = { IconTile(Ic.Edit) })
                    RowDivider()
                    ListRow(stringResource(R.string.template_move), t.folder.ifEmpty { stringResource(R.string.folder_default) }, onClick = { moving = t; acting = null }, leading = { IconTile(Ic.Folder) })
                    RowDivider()
                    ListRow(stringResource(R.string.delete), onClick = { deleting = t; acting = null }, leading = { IconTile(Ic.Trash, Palette.red, Palette.amberTint) })
                }
            }
        }
    }
    moving?.let { t ->
        Sheet(stringResource(R.string.template_move), { moving = null }) {
            Group {
                (listOf("") + folders).forEachIndexed { i, f ->
                    if (i > 0) RowDivider()
                    ListRow(
                        f.ifEmpty { stringResource(R.string.folder_default) },
                        onClick = {
                            if (f != t.folder) state.moveTemplate(t, folder = f)
                            moving = null
                        },
                        leading = { IconTile(Ic.Folder, if (f == t.folder) Palette.primary else Palette.text2, if (f == t.folder) Palette.primaryTint else Palette.bg) },
                    )
                }
            }
        }
    }
    renaming?.let { t ->
        TextDialog(stringResource(R.string.rename), stringResource(R.string.template_name), t.name, stringResource(R.string.save), { renaming = null }) {
            if (it != t.name) state.moveTemplate(t, name = it)
            renaming = null
        }
    }
    if (newFolder) {
        TextDialog(stringResource(R.string.folder_new), stringResource(R.string.folder_name), "", stringResource(R.string.create), { newFolder = false }) {
            state.templateFolder("create", it)
            newFolder = false
        }
    }
    renameFolder?.let { f ->
        TextDialog(stringResource(R.string.folder_rename), stringResource(R.string.folder_name), f, stringResource(R.string.save), { renameFolder = null }) {
            if (it != f) state.templateFolder("rename", f, to = it)
            if (filter == f) filter = it
            renameFolder = null
        }
    }
    deleteFolder?.let { f ->
        val count = state.templates.count { !it.starter && it.folder == f }
        ConfirmDialog(
            stringResource(R.string.folder_delete_title),
            if (count == 0) stringResource(R.string.folder_delete_empty, f) else pluralStringResource(R.plurals.folder_delete_body, count, f, count),
            stringResource(R.string.delete),
            danger = true,
            onDismiss = { deleteFolder = null },
        ) {
            state.templateFolder("delete", f, withTemplates = count > 0)
            deleteFolder = null
        }
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
                                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                    Text(sizeText(t.widthMm, t.heightMm), style = Type.caption)
                                    if (t.fits == true) {
                                        Text(stringResource(R.string.template_fits), style = Type.caption.copy(color = Palette.primary), maxLines = 1)
                                    }
                                }
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
    "starter:spice" to R.string.template_spice,
    "starter:freezer-bag" to R.string.template_freezer_bag,
    "starter:leftovers" to R.string.template_leftovers,
    "starter:meal-prep" to R.string.template_meal_prep,
    "starter:pantry-bin" to R.string.template_pantry_bin,
    "starter:bake-sale" to R.string.template_bake_sale,
    "starter:allergens" to R.string.template_allergens,
    "starter:opened-on" to R.string.template_opened_on,
    "starter:recipe-qr" to R.string.template_recipe_qr,
    "starter:file-folder" to R.string.template_file_folder,
    "starter:binder-spine" to R.string.template_binder_spine,
    "starter:shelf-label" to R.string.template_shelf_label,
    "starter:desk-name" to R.string.template_desk_name,
    "starter:inventory-bin" to R.string.template_inventory_bin,
    "starter:key-tag" to R.string.template_key_tag,
    "starter:return-to-owner" to R.string.template_return_to_owner,
    "starter:cable-wrap" to R.string.template_cable_wrap,
    "starter:patch-panel" to R.string.template_patch_panel,
    "starter:power-adapter" to R.string.template_power_adapter,
    "starter:network-port" to R.string.template_network_port,
    "starter:breaker-panel" to R.string.template_breaker_panel,
    "starter:server-rack" to R.string.template_server_rack,
    "starter:charger" to R.string.template_charger,
    "starter:outlet" to R.string.template_outlet,
    "starter:return-address" to R.string.template_return_address,
    "starter:fragile" to R.string.template_fragile,
    "starter:this-way-up" to R.string.template_this_way_up,
    "starter:keep-cold" to R.string.template_keep_cold,
    "starter:parcel-contents" to R.string.template_parcel_contents,
    "starter:box-of" to R.string.template_box_of,
    "starter:moving-box" to R.string.template_moving_box,
    "starter:order-tracking" to R.string.template_order_tracking,
    "starter:conference-badge" to R.string.template_conference_badge,
    "starter:table-number" to R.string.template_table_number,
    "starter:place-card" to R.string.template_place_card,
    "starter:gift-tag" to R.string.template_gift_tag,
    "starter:party-favor" to R.string.template_party_favor,
    "starter:raffle-ticket" to R.string.template_raffle_ticket,
    "starter:wedding-favor" to R.string.template_wedding_favor,
    "starter:coat-check" to R.string.template_coat_check,
)

/** Starter categories, in the order of `template.list`. */
private val CATEGORIES = listOf(
    "kitchen" to R.string.template_category_kitchen,
    "office" to R.string.template_category_office,
    "cables" to R.string.template_category_cables,
    "shipping" to R.string.template_category_shipping,
    "events" to R.string.template_category_events,
)

/** Starter names are translated like in the studio; library templates keep their own name. */
fun Template.displayName(context: Context): String = STARTER_NAMES[ref]?.let(context::getString) ?: name

@Composable
fun templateName(t: Template): String = t.displayName(LocalContext.current)
