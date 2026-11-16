package org.labely.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/** Colours of `design/labely-mobile.figcraft` (`C` in design/gen.py). */
object Palette {
    val primary = Color(0xFF4C6EF5)
    val primaryDark = Color(0xFF3B5BDB)
    val primaryTint = Color(0xFFEDF2FF)
    val primaryTint2 = Color(0xFFDBE4FF)
    val bg = Color(0xFFF5F6F8)
    val surface = Color(0xFFFFFFFF)
    val border = Color(0xFFE4E7EC)
    val borderSoft = Color(0xFFEEF0F3)
    val text = Color(0xFF1B2130)
    val text2 = Color(0xFF475066)
    val muted = Color(0xFF8A93A6)
    val canvas = Color(0xFFE8EBF0)
    val field = Color(0xFFE9ECF1)
    val green = Color(0xFF12B886)
    val greenTint = Color(0xFFE6FCF5)
    val amber = Color(0xFFF59F00)
    val amberTint = Color(0xFFFFF4E0)
    val red = Color(0xFFFA5252)
}

/** Type scale of the mobile design (Inter there; the platform sans here). */
object Type {
    val largeTitle = TextStyle(fontSize = 30.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, color = Palette.text)
    val title = TextStyle(fontSize = 22.sp, fontWeight = FontWeight.Bold, color = Palette.text)
    val section = TextStyle(fontSize = 17.sp, fontWeight = FontWeight.Bold, color = Palette.text)
    val nav = TextStyle(fontSize = 16.sp, fontWeight = FontWeight.Bold, color = Palette.text)
    val body = TextStyle(fontSize = 15.sp, color = Palette.text)
    val bodyStrong = TextStyle(fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = Palette.text)
    val caption = TextStyle(fontSize = 13.sp, color = Palette.muted)
    val small = TextStyle(fontSize = 11.sp, color = Palette.muted)
}

@Composable
fun LabelyTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = lightColorScheme(
            primary = Palette.primary,
            onPrimary = Color.White,
            primaryContainer = Palette.primaryTint,
            onPrimaryContainer = Palette.primaryDark,
            background = Palette.bg,
            onBackground = Palette.text,
            surface = Palette.surface,
            onSurface = Palette.text,
            surfaceVariant = Palette.field,
            onSurfaceVariant = Palette.text2,
            outline = Palette.border,
            outlineVariant = Palette.borderSoft,
            error = Palette.red,
        ),
        typography = Typography(bodyLarge = Type.body, bodyMedium = Type.body, labelLarge = Type.bodyStrong),
        content = content,
    )
}
