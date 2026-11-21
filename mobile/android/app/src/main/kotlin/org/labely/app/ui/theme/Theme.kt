package org.labely.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/** The colour tokens of `design/labely-mobile.figcraft` (`C` in design/gen.py), light or dark. */
@Immutable
data class LabelyColors(
    val dark: Boolean,
    val primary: Color,
    /** Accent text on tints; in light also the deep end of the primary gradient. */
    val primaryDark: Color,
    val primaryTint: Color,
    val primaryTint2: Color,
    val bg: Color,
    val surface: Color,
    val border: Color,
    val borderSoft: Color,
    val text: Color,
    val text2: Color,
    val muted: Color,
    val canvas: Color,
    val field: Color,
    val green: Color,
    val greenTint: Color,
    val amber: Color,
    val amberTint: Color,
    val red: Color,
) {
    companion object {
        val Light = LabelyColors(
            dark = false,
            primary = Color(0xFF4C6EF5),
            primaryDark = Color(0xFF3B5BDB),
            primaryTint = Color(0xFFEDF2FF),
            primaryTint2 = Color(0xFFDBE4FF),
            bg = Color(0xFFF5F6F8),
            surface = Color(0xFFFFFFFF),
            border = Color(0xFFE4E7EC),
            borderSoft = Color(0xFFEEF0F3),
            text = Color(0xFF1B2130),
            text2 = Color(0xFF475066),
            muted = Color(0xFF8A93A6),
            canvas = Color(0xFFE8EBF0),
            field = Color(0xFFE9ECF1),
            green = Color(0xFF12B886),
            greenTint = Color(0xFFE6FCF5),
            amber = Color(0xFFF59F00),
            amberTint = Color(0xFFFFF4E0),
            red = Color(0xFFFA5252),
        )

        /** The same tokens in dark; the values match the desktop and iOS. */
        val Dark = LabelyColors(
            dark = true,
            primary = Color(0xFF5C7CFA),
            primaryDark = Color(0xFF91A7FF),
            primaryTint = Color(0xFF1E2640),
            primaryTint2 = Color(0xFF28345C),
            bg = Color(0xFF14171D),
            surface = Color(0xFF1C2028),
            border = Color(0xFF2E3440),
            borderSoft = Color(0xFF262B35),
            text = Color(0xFFE6E9EF),
            text2 = Color(0xFFB4BCCB),
            muted = Color(0xFF8790A3),
            canvas = Color(0xFF101318),
            field = Color(0xFF252A34),
            green = Color(0xFF20C997),
            greenTint = Color(0xFF0F2E26),
            amber = Color(0xFFFAB005),
            amberTint = Color(0xFF33270D),
            red = Color(0xFFFF6B6B),
        )
    }
}

/** The type scale of the mobile design (Inter there; the platform sans here), in [colors]. */
@Immutable
class LabelyType(colors: LabelyColors) {
    val largeTitle = TextStyle(fontSize = 30.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, color = colors.text)
    val title = TextStyle(fontSize = 22.sp, fontWeight = FontWeight.Bold, color = colors.text)
    val section = TextStyle(fontSize = 17.sp, fontWeight = FontWeight.Bold, color = colors.text)
    val nav = TextStyle(fontSize = 16.sp, fontWeight = FontWeight.Bold, color = colors.text)
    val body = TextStyle(fontSize = 15.sp, color = colors.text)
    val bodyStrong = TextStyle(fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = colors.text)
    val caption = TextStyle(fontSize = 13.sp, color = colors.muted)
    val small = TextStyle(fontSize = 11.sp, color = colors.muted)

    companion object {
        val Light = LabelyType(LabelyColors.Light)
        val Dark = LabelyType(LabelyColors.Dark)
    }
}

val LocalLabelyColors = staticCompositionLocalOf { LabelyColors.Light }
val LocalLabelyType = staticCompositionLocalOf { LabelyType.Light }

/**
 * The colours of the current [LabelyTheme]. Read them in composition, like
 * `MaterialTheme.colorScheme`; a draw lambda captures them from there.
 */
object Palette {
    val current: LabelyColors @Composable @ReadOnlyComposable get() = LocalLabelyColors.current
    val dark: Boolean @Composable @ReadOnlyComposable get() = current.dark
    val primary: Color @Composable @ReadOnlyComposable get() = current.primary
    val primaryDark: Color @Composable @ReadOnlyComposable get() = current.primaryDark
    val primaryTint: Color @Composable @ReadOnlyComposable get() = current.primaryTint
    val primaryTint2: Color @Composable @ReadOnlyComposable get() = current.primaryTint2
    val bg: Color @Composable @ReadOnlyComposable get() = current.bg
    val surface: Color @Composable @ReadOnlyComposable get() = current.surface
    val border: Color @Composable @ReadOnlyComposable get() = current.border
    val borderSoft: Color @Composable @ReadOnlyComposable get() = current.borderSoft
    val text: Color @Composable @ReadOnlyComposable get() = current.text
    val text2: Color @Composable @ReadOnlyComposable get() = current.text2
    val muted: Color @Composable @ReadOnlyComposable get() = current.muted
    val canvas: Color @Composable @ReadOnlyComposable get() = current.canvas
    val field: Color @Composable @ReadOnlyComposable get() = current.field
    val green: Color @Composable @ReadOnlyComposable get() = current.green
    val greenTint: Color @Composable @ReadOnlyComposable get() = current.greenTint
    val amber: Color @Composable @ReadOnlyComposable get() = current.amber
    val amberTint: Color @Composable @ReadOnlyComposable get() = current.amberTint
    val red: Color @Composable @ReadOnlyComposable get() = current.red
}

/** The text styles of the current [LabelyTheme], coloured for it. */
object Type {
    val largeTitle: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.largeTitle
    val title: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.title
    val section: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.section
    val nav: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.nav
    val body: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.body
    val bodyStrong: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.bodyStrong
    val caption: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.caption
    val small: TextStyle @Composable @ReadOnlyComposable get() = LocalLabelyType.current.small
}

/** Settings → Appearance; [System] follows the system's dark theme. */
enum class Appearance(val id: String) {
    System("system"),
    Light("light"),
    Dark("dark"),
    ;

    companion object {
        fun of(id: String?): Appearance = entries.firstOrNull { it.id == id } ?: System
    }
}

/** Whether [appearance] is dark here. */
@Composable
@ReadOnlyComposable
fun Appearance.isDark(): Boolean =
    when (this) {
        Appearance.System -> isSystemInDarkTheme()
        Appearance.Light -> false
        Appearance.Dark -> true
    }

private fun LabelyColors.scheme(): ColorScheme =
    if (dark) {
        darkColorScheme(
            primary = primary,
            onPrimary = Color.White,
            primaryContainer = primaryTint,
            onPrimaryContainer = primaryDark,
            background = bg,
            onBackground = text,
            surface = surface,
            onSurface = text,
            surfaceVariant = field,
            onSurfaceVariant = text2,
            surfaceContainerLow = surface,
            surfaceContainer = surface,
            surfaceContainerHigh = surface,
            outline = border,
            outlineVariant = borderSoft,
            error = red,
        )
    } else {
        lightColorScheme(
            primary = primary,
            onPrimary = Color.White,
            primaryContainer = primaryTint,
            onPrimaryContainer = primaryDark,
            background = bg,
            onBackground = text,
            surface = surface,
            onSurface = text,
            surfaceVariant = field,
            onSurfaceVariant = text2,
            outline = border,
            outlineVariant = borderSoft,
            error = red,
        )
    }

@Composable
fun LabelyTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val colors = if (dark) LabelyColors.Dark else LabelyColors.Light
    val type = if (dark) LabelyType.Dark else LabelyType.Light
    val scheme = remember(colors) { colors.scheme() }
    val typography = remember(type) { Typography(bodyLarge = type.body, bodyMedium = type.body, labelLarge = type.bodyStrong) }
    CompositionLocalProvider(LocalLabelyColors provides colors, LocalLabelyType provides type) {
        MaterialTheme(
            colorScheme = scheme,
            typography = typography,
            content = content,
        )
    }
}
