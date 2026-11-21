package org.labely.app.ui.theme

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The stroke icons of the mobile design (`ICONS` in design/gen.py): 24-unit paths, round caps.
 * Only the ones the app uses are copied.
 */
enum class Ic(val path: String) {
    Home("M3 11l9-7 9 7 M5 9.5V20h14V9.5"),
    Templates("M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z"),
    Clock("M12 3a9 9 0 1 0 0.01 0 M12 7v5l3 2"),
    Printer("M7 9V3h10v6 M4 9h16v8H4z M7 14h10v7H7z"),
    Settings("M4 6h9 M17 6h3 M15 4v4 M4 12h3 M11 12h9 M9 10v4 M4 18h11 M19 18h1 M17 16v4"),
    Plus("M12 5v14 M5 12h14"),
    Minus("M6 12h12"),
    Search("M11 4a7 7 0 1 0 0.01 0 M16 16l4 4"),
    ChevLeft("M15 6l-6 6 6 6"),
    ChevRight("M9 6l6 6-6 6"),
    ChevDown("M6 9l6 6 6-6"),
    Close("M6 6l12 12 M18 6L6 18"),
    Check("M5 12l5 5 9-10"),
    Warn("M12 4l9 16H3z M12 10v4 M12 17h.01"),
    Undo("M9 14L4 9l5-5 M4 9h10a5.5 5.5 0 0 1 0 11h-3"),
    Redo("M15 14l5-5-5-5 M20 9H10a5.5 5.5 0 0 0 0 11h3"),
    Trash("M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13 M10 11v6 M14 11v6"),
    Copy("M8 8h12v12H8z M4 16V4h12"),
    Layers("M12 3l9 5-9 5-9-5z M3 13l9 5 9-5"),
    Text("M5 7V5h14v2 M12 5v14 M9 19h6"),
    Barcode("M4 5v14 M7 5v14 M10 5v14 M14 5v14 M16 5v14 M20 5v14"),
    Qr("M4 4h6v6H4z M14 4h6v6h-6z M4 14h6v6H4z M14 14h2v2h-2z M18 18h2v2h-2z M18 14h2 M14 19h1"),
    Rect("M4 6h16v12H4z"),
    Ellipse("M4 12a8 6.5 0 1 0 16 0a8 6.5 0 1 0 -16 0"),
    Line("M5 19L19 5"),
    Image("M4 5h16v14H4z M4 16l5-5 4 4 3-3 4 4 M15 8a1.5 1.5 0 1 0 0.01 0"),
    Table("M4 5h16v14H4z M4 10h16 M4 15h16 M10 5v14"),
    Border("M3 6h18v12H3z M6 9h12v6H6z"),
    Bold("M7 5h6a3.5 3.5 0 0 1 0 7H7z M7 12h7a3.5 3.5 0 0 1 0 7H7z"),
    TLeft("M4 6h16 M4 10h10 M4 14h16 M4 18h10"),
    TCenter("M4 6h16 M7 10h10 M4 14h16 M7 18h10"),
    TRight("M4 6h16 M10 10h10 M4 14h16 M10 18h10"),
    Up("M12 19V5 M6 11l6-6 6 6"),
    Down("M12 5v14 M6 13l6 6 6-6"),
    Bluetooth("M7 7l10 10-5 4V3l5 4L7 17"),
    Scan("M4 8V4h4 M16 4h4v4 M20 16v4h-4 M8 20H4v-4 M4 12h16"),
    Data("M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3z M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6 M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"),
    Open("M3 7V5h6l2 2h10v12H3z M3 11h18"),
    Label("M3 6h18v12H3z M6 9h12v6H6z"),
    Folder("M3 6h6l2 2h10v11H3z"),
    Download("M12 4v12 M7 11l5 5 5-5 M4 20h16"),
    Upload("M12 16V4 M7 9l5-5 5 5 M4 20h16"),
    File("M6 3h8l4 4v14H6z M14 3v4h4"),
    Link("M10 14a4 4 0 0 0 6 0l3-3a4 4 0 0 0-6-6l-1 1 M14 10a4 4 0 0 0-6 0l-3 3a4 4 0 0 0 6 6l1-1"),
    Contrast("M12 3a9 9 0 1 0 0.01 0 M12 3v18 M12 8h7.5 M12 12h9 M12 16h7.5"),
    Info("M12 3a9 9 0 1 0 0.01 0 M12 11v6 M12 7.5h.01"),
    Lock("M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3"),
    Unlock("M6 11h12v9H6z M8 11V8a4 4 0 0 1 7.5-2"),
    Eye("M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z M12 9a3 3 0 1 0 0.01 0"),
    EyeOff("M4 4l16 16 M6.5 7.5C3.8 9.3 2 12 2 12s4 7 10 7c1.8 0 3.4-.6 4.8-1.4 M10 5.2c.6-.1 1.3-.2 2-.2 6 0 10 7 10 7s-.9 1.6-2.5 3.2"),
    Rotate("M20 12a8 8 0 1 1-2.6-5.9 M20 4v5h-5"),
    FlipH("M12 3v18 M9 7L4 12l5 5z M15 7l5 5-5 5z"),
    FlipV("M3 12h18 M7 9l5-5 5 5z M7 15l5 5 5-5z"),
    Group("M4 4h4 M4 4v4 M20 4h-4 M20 4v4 M4 20h4 M4 20v-4 M20 20h-4 M20 20v-4 M8 8h8v8H8z"),
    Ungroup("M4 4h4 M4 4v4 M20 20h-4 M20 20v-4 M7 7h6v6H7z M11 11h6v6h-6z"),
    AlignLeft("M4 4v16 M8 7h10v3H8z M8 14h6v3H8z"),
    AlignCenter("M12 4v16 M6 7h12v3H6z M8 14h8v3H8z"),
    AlignRight("M20 4v16 M6 7h10v3H6z M10 14h6v3h-6z"),
    AlignTop("M4 4h16 M7 8h3v10H7z M14 8h3v6h-3z"),
    AlignMiddle("M4 12h16 M7 6h3v12H7z M14 8h3v8h-3z"),
    AlignBottom("M4 20h16 M7 6h3v10H7z M14 10h3v6h-3z"),
    DistH("M4 4v16 M20 4v16 M9 8h6v8H9z"),
    DistV("M4 4h16 M4 20h16 M8 9h8v6H8z"),
    Front("M12 20V7 M6 13l6-6 6 6 M5 3h14"),
    Back("M12 4v13 M6 11l6 6 6-6 M5 21h14"),
    Paste("M8 5H5v16h14V5h-3 M8 3h8v4H8z"),
    Cut("M6 6a2.5 2.5 0 1 0 0.01 0 M6 18a2.5 2.5 0 1 0 0.01 0 M8 8l12 10 M8 16L20 6"),
    Italic("M10 5h8 M6 19h8 M14 5l-4 14"),
    Underline("M7 4v7a5 5 0 0 0 10 0V4 M5 20h14"),
    More("M5 12h.01 M12 12h.01 M19 12h.01"),
    Calendar("M4 6h16v14H4z M4 10h16 M8 3v4 M16 3v4"),
    Hash("M9 4L7 20 M17 4l-2 16 M4 9h16 M4 15h16"),
    Pause("M8 5v14 M16 5v14"),
    Play("M7 5l12 7-12 7z"),
    Grid("M4 4h16v16H4z M4 9.3h16 M4 14.6h16 M9.3 4v16 M14.6 4v16"),
    Edit("M4 20h4L19 9l-4-4L4 16z M13 7l4 4"),
    Star("M12 3l2.7 5.6 6.1.8-4.5 4.3 1.1 6.1L12 16.9 6.6 19.8l1.1-6.1L3.2 9.4l6.1-.8z"),
    Network("M4 9a12 12 0 0 1 16 0 M7 13a7 7 0 0 1 10 0 M12 17h.01"),
    Justify("M4 6h16 M4 10h16 M4 14h16 M4 18h16"),
    Left("M19 12H5 M11 6l-6 6 6 6"),
    Right("M5 12h14 M13 6l6 6-6 6"),
    ;

    val vector: ImageVector by lazy {
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
            .addPath(
                pathData = PathParser().parsePathString(path).toNodes(),
                fill = null,
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 1.8f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
            .build()
    }
}

@Composable
fun LIcon(icon: Ic, tint: Color = Palette.text2, size: Dp = 22.dp, @Suppress("ModifierParameter") modifier: Modifier = Modifier) {
    Icon(icon.vector, contentDescription = null, tint = tint, modifier = modifier.size(size))
}
