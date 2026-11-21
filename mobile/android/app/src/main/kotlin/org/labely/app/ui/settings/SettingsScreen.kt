package org.labely.app.ui.settings

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import org.labely.app.AppState
import org.labely.app.editor.CanvasAids
import org.labely.app.editor.Sheet
import org.labely.app.editor.Units
import org.labely.app.ui.SectionHeader
import org.labely.app.ui.Segmented
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
import org.labely.app.ui.theme.Appearance
import org.labely.app.ui.theme.Ic
import org.labely.app.ui.theme.LIcon
import org.labely.app.ui.theme.Palette
import org.labely.app.ui.theme.Type

/**
 * M10 · Beállítások, reduced to what the shell has: language (the system's per-app language),
 * app permissions, whether assistants may print through AppFunctions (Android 16+, ADR-0028) and
 * version information.
 */
@Composable
fun SettingsScreen(state: AppState) {
    val context = LocalContext.current
    val appSettings = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
    val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull().orEmpty()
    var about by remember { mutableStateOf(false) }
    if (about) {
        Sheet(stringResource(R.string.settings_about), { about = false }) {
            Text(stringResource(R.string.app_name) + " " + version, style = Type.title)
            Text(stringResource(R.string.about_body), style = Type.body)
            Group {
                ListRow(stringResource(R.string.settings_version)) { Text(version, style = Type.caption) }
                RowDivider()
                ListRow(stringResource(R.string.settings_engine)) { Text(stringResource(R.string.engine_version, state.engine.apiVersion), style = Type.caption) }
                RowDivider()
                ListRow(stringResource(R.string.about_licence), stringResource(R.string.about_licence_sub))
            }
        }
    }
    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item { LargeTitle(stringResource(R.string.tab_settings)) }
        item {
            Group {
                ListRow(
                    stringResource(R.string.settings_language),
                    stringResource(R.string.settings_language_sub),
                    onClick = {
                        val intent =
                            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
                                Intent(Settings.ACTION_APP_LOCALE_SETTINGS, Uri.fromParts("package", context.packageName, null))
                            } else {
                                appSettings
                            }
                        runCatching { context.startActivity(intent) }
                    },
                    leading = { IconTile(Ic.Info) },
                ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
                RowDivider()
                ListRow(
                    stringResource(R.string.settings_permissions),
                    stringResource(R.string.settings_permissions_sub),
                    onClick = { runCatching { context.startActivity(appSettings) } },
                    leading = { IconTile(Ic.Bluetooth) },
                ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
                if (android.os.Build.VERSION.SDK_INT >= AGENT_API) {
                    RowDivider()
                    ListRow(
                        stringResource(R.string.settings_agent_print),
                        stringResource(R.string.settings_agent_print_sub),
                        onClick = { state.allowAgentPrint = !state.allowAgentPrint },
                        leading = { IconTile(Ic.Printer) },
                    ) { Switch(state.allowAgentPrint, onCheckedChange = { state.allowAgentPrint = it }) }
                }
            }
        }
        item {
            Group {
                // Every size field and size shown follows the unit.
                ListRow(stringResource(R.string.settings_units), stringResource(R.string.settings_units_sub), leading = { IconTile(Ic.Rect) }) {
                    Segmented(listOf("mm", "inch"), state.units.ordinal) { state.units = Units.entries[it] }
                }
                RowDivider()
                ListRow(stringResource(R.string.settings_appearance), leading = { IconTile(Ic.Contrast) }) {
                    Segmented(
                        listOf(
                            stringResource(R.string.appearance_system),
                            stringResource(R.string.appearance_light),
                            stringResource(R.string.appearance_dark),
                        ),
                        state.appearance.ordinal,
                        scroll = true,
                    ) { state.appearance = Appearance.entries[it] }
                }
            }
        }
        item { SectionHeader(stringResource(R.string.settings_canvas)) }
        item { CanvasAids(state) }
        item {
            Group {
                ListRow(
                    stringResource(R.string.settings_about),
                    stringResource(R.string.settings_about_sub, version),
                    onClick = { about = true },
                    leading = { IconTile(Ic.Info) },
                ) { LIcon(Ic.ChevRight, Palette.muted, 18.dp) }
                RowDivider()
                ListRow(stringResource(R.string.settings_version), leading = { IconTile(Ic.Label) }) { Text(version, style = Type.caption) }
                RowDivider()
                ListRow(stringResource(R.string.settings_engine), leading = { IconTile(Ic.Data) }) {
                    Text(stringResource(R.string.engine_version, state.engine.apiVersion), style = Type.caption)
                }
            }
        }
    }
}

/** AppFunctions exist from Android 16 (API 36). */
private const val AGENT_API = 36
