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
import org.labely.app.AppState
import org.labely.app.R
import org.labely.app.ui.Group
import org.labely.app.ui.IconTile
import org.labely.app.ui.LargeTitle
import org.labely.app.ui.ListRow
import org.labely.app.ui.RowDivider
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
