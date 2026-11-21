package org.labely.app

import android.app.Application
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.labely.app.ble.AndroidBle
import org.labely.app.editor.EditorScreen
import org.labely.app.editor.LabelSheet
import org.labely.app.engine.Engine
import org.labely.app.ui.history.HistoryScreen
import org.labely.app.ui.home.HomeScreen
import org.labely.app.ui.printers.PrintersScreen
import org.labely.app.ui.settings.SettingsScreen
import org.labely.app.ui.templates.TemplatesScreen
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.LabelyTheme
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/** Holds the engine and the UI state for the life of the process (ADR-0027). */
class LabelyApplication : Application() {
    val state: AppState by lazy {
        AppState(this, Engine(filesDir.path, AndroidBle(this)), CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate))
            .also { it.start() }
    }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // The design is light only, so the bar icons stay dark even in system dark mode.
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        val state = (application as LabelyApplication).state
        setContent { LabelyTheme { App(state) } }
    }
}

@Composable
private fun App(state: AppState) {
    val snackbar = remember { SnackbarHostState() }
    var newLabel by remember { mutableStateOf(false) }
    val openFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri -> uri?.let(state::openUri) }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            state.message = null
        }
    }

    Box(Modifier.fillMaxSize().background(Palette.bg)) {
        val editor = state.editor
        if (editor != null) {
            EditorScreen(editor, state::closeEditor)
        } else {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f).statusBarsPadding()) {
                    when (state.tab) {
                        Tab.Home -> HomeScreen(state, onNewLabel = { newLabel = true }, onOpenFile = { openFile.launch(arrayOf("*/*")) })
                        Tab.Templates -> TemplatesScreen(state, onNewLabel = { newLabel = true })
                        Tab.History -> HistoryScreen(state)
                        Tab.Printers -> PrintersScreen(state)
                        Tab.Settings -> SettingsScreen(state)
                    }
                }
                TabBar(state.tab) { state.tab = it }
            }
        }
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 72.dp))
    }

    if (newLabel) {
        LabelSheet(
            title = stringResource(R.string.new_label),
            confirm = stringResource(R.string.create),
            initial = Triple(PRESET_SIZES[0].first, PRESET_SIZES[0].second, "gap"),
            onDismiss = { newLabel = false },
            detect = if (state.printer != null) ({ state.detectPaper() }) else null,
        ) { w, h, media ->
            newLabel = false
            state.newLabel(w, h, media)
        }
    }
}

@Composable
private fun TabBar(current: Tab, onSelect: (Tab) -> Unit) {
    Column(Modifier.fillMaxWidth().background(Palette.surface).navigationBarsPadding()) {
        HorizontalDivider(color = Palette.border)
        Row(Modifier.fillMaxWidth().height(60.dp)) {
            TabItem(Ic.Home, stringResource(R.string.tab_home), current == Tab.Home) { onSelect(Tab.Home) }
            TabItem(Ic.Templates, stringResource(R.string.tab_templates), current == Tab.Templates) { onSelect(Tab.Templates) }
            TabItem(Ic.Clock, stringResource(R.string.tab_history), current == Tab.History) { onSelect(Tab.History) }
            TabItem(Ic.Printer, stringResource(R.string.tab_printers), current == Tab.Printers) { onSelect(Tab.Printers) }
            TabItem(Ic.Settings, stringResource(R.string.tab_settings), current == Tab.Settings) { onSelect(Tab.Settings) }
        }
    }
}

@Composable
private fun RowScope.TabItem(icon: Ic, label: String, selected: Boolean, onClick: () -> Unit) {
    Column(
        Modifier.weight(1f).fillMaxSize().clickable(onClick = onClick).padding(top = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        LIcon(icon, if (selected) Palette.primary else Palette.muted, 24.dp)
        Text(
            label,
            style = Type.small.copy(
                fontSize = 11.sp,
                color = if (selected) Palette.primaryDark else Palette.muted,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
            ),
            maxLines = 1,
        )
    }
}
