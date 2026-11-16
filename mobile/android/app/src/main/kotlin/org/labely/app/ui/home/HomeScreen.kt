package org.labely.app.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.labely.app.AppState
import org.labely.app.PRESET_SIZES
import org.labely.app.R
import org.labely.app.Tab
import org.labely.app.ui.Card
import org.labely.app.ui.Chip
import org.labely.app.ui.IconTile
import org.labely.app.ui.SearchField
import org.labely.app.ui.SectionHeader
import org.labely.app.ui.ThumbTile
import org.labely.app.ui.sizeText
import org.labely.app.ui.templates.TemplateGrid
import org.labely.app.ui.templates.displayName
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** M1 · Kezdőlap: quick start, starter templates and recent labels. */
@Composable
fun HomeScreen(state: AppState, onNewLabel: () -> Unit, onOpenFile: () -> Unit) {
    var query by remember { mutableStateOf("") }
    val context = LocalContext.current
    val starters = state.templates.filter { it.starter && it.displayName(context).contains(query, ignoreCase = true) }
    val recents = state.recents.filter { it.name.contains(query, ignoreCase = true) }
    LazyColumn(
        Modifier.fillMaxWidth(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item { Header(state) }
        item {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.home_greeting), style = Type.body.copy(color = Palette.muted))
                Text(stringResource(R.string.home_title), style = Type.largeTitle.copy(fontSize = 26.sp))
            }
        }
        item { SearchField(query, stringResource(R.string.home_search)) { query = it } }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                ActionCard(Ic.Plus, stringResource(R.string.home_new), stringResource(R.string.home_new_sub), primary = true, Modifier.weight(1f), onNewLabel)
                ActionCard(Ic.Templates, stringResource(R.string.home_from_template), stringResource(R.string.home_from_template_sub), false, Modifier.weight(1f)) {
                    state.tab = Tab.Templates
                }
                ActionCard(Ic.Open, stringResource(R.string.home_open), stringResource(R.string.home_open_sub), false, Modifier.weight(1f), onOpenFile)
            }
        }
        item {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PRESET_SIZES.forEachIndexed { i, (w, h) -> Chip(sizeText(w, h), i == 0) { state.newLabel(w, h) } }
            }
        }
        item { SectionHeader(stringResource(R.string.home_starters), stringResource(R.string.see_all)) { state.tab = Tab.Templates } }
        item { TemplateGrid(starters.take(4), state, onOpen = state::fromTemplate) }
        if (recents.isNotEmpty()) {
            item { SectionHeader(stringResource(R.string.home_recent)) }
            items(recents, key = { it.path }) { r ->
                Card(Modifier.fillMaxWidth(), onClick = { state.openRecent(r) }) {
                    Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.size(width = 96.dp, height = 60.dp)) { ThumbTile(state.thumbs[r.path], r.widthMm, r.heightMm) }
                        Column(Modifier.weight(1f)) {
                            Text(r.name, style = Type.bodyStrong, maxLines = 1)
                            Text(sizeText(r.widthMm, r.heightMm), style = Type.caption)
                        }
                        LIcon(Ic.ChevRight, Palette.muted, 18.dp)
                    }
                }
            }
        }
        item { Spacer(Modifier.height(8.dp)) }
    }
}

@Composable
private fun Header(state: AppState) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.size(width = 26.dp, height = 20.dp).clip(RoundedCornerShape(5.dp)).background(Palette.primary), contentAlignment = Alignment.CenterStart) {
            Row(Modifier.padding(start = 5.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                Box(Modifier.size(4.dp).clip(CircleShape).background(Color.White))
                Box(Modifier.size(width = 9.dp, height = 3.dp).clip(RoundedCornerShape(2.dp)).background(Color.White))
            }
        }
        Text(stringResource(R.string.app_name), style = Type.title.copy(fontSize = 24.sp), modifier = Modifier.weight(1f))
        PrinterPill(state)
    }
}

/** Connected printer and battery, or a prompt to connect one. */
@Composable
fun PrinterPill(state: AppState) {
    val p = state.printer
    val battery = state.printerStatus?.batteryPercent
    Row(
        Modifier.clip(RoundedCornerShape(50)).background(if (p != null) Palette.greenTint else Palette.field)
            .clickable { state.tab = Tab.Printers }.padding(horizontal = 12.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(if (p != null) Palette.green else Palette.muted))
        Text(p?.model?.name ?: p?.name ?: stringResource(R.string.no_printer), style = Type.bodyStrong.copy(fontSize = 13.sp), maxLines = 1)
        if (battery != null) Text("· $battery%", style = Type.caption)
    }
}

@Composable
private fun ActionCard(icon: Ic, title: String, sub: String, primary: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val shape = RoundedCornerShape(18.dp)
    val bg =
        if (primary) {
            Modifier.background(Brush.verticalGradient(listOf(Palette.primary, Palette.primaryDark)))
        } else {
            Modifier.background(Palette.surface).border(1.dp, Palette.border, shape)
        }
    Column(
        modifier.height(150.dp).clip(shape).then(bg)
            .clickable(onClick = onClick).padding(14.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        IconTile(icon, if (primary) Color.White else Palette.primary, if (primary) Color.White.copy(alpha = 0.2f) else Palette.primaryTint)
        Column {
            Text(title, style = Type.bodyStrong.copy(fontSize = 16.sp, color = if (primary) Color.White else Palette.text, fontWeight = FontWeight.Bold), maxLines = 1)
            Text(sub, style = Type.caption.copy(color = if (primary) Color.White.copy(alpha = 0.85f) else Palette.muted), maxLines = 2)
        }
    }
}
