package org.labely.app.ui.theme

import android.content.Context
import androidx.compose.ui.graphics.Color
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.labely.app.LabelyApplication
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The light and dark tokens, the type that follows them and the saved appearance. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ThemeTest {
    @Test
    fun darkTokensMatchTheOtherPlatforms() {
        val d = LabelyColors.Dark
        assertTrue(d.dark)
        assertEquals(Color(0xFF5C7CFA), d.primary)
        assertEquals(Color(0xFF91A7FF), d.primaryDark)
        assertEquals(Color(0xFF14171D), d.bg)
        assertEquals(Color(0xFF1C2028), d.surface)
        assertEquals(Color(0xFFE6E9EF), d.text)
        assertEquals(Color(0xFF101318), d.canvas)
        assertEquals(Color(0xFFFF6B6B), d.red)
        assertFalse(LabelyColors.Light.dark)
        assertEquals(Color(0xFF4C6EF5), LabelyColors.Light.primary)
    }

    @Test
    fun typeIsColouredByItsPalette() {
        assertEquals(LabelyColors.Dark.text, LabelyType.Dark.body.color)
        assertEquals(LabelyColors.Dark.muted, LabelyType.Dark.caption.color)
        assertEquals(LabelyColors.Light.text, LabelyType.Light.title.color)
    }

    @Test
    fun appearanceParsesAndDefaultsToSystem() {
        for (a in Appearance.entries) assertEquals(a, Appearance.of(a.id))
        assertEquals(Appearance.System, Appearance.of(null))
        assertEquals(Appearance.System, Appearance.of("sepia"))
    }

    @Test
    fun appearanceIsSaved() {
        val context: Context = ApplicationProvider.getApplicationContext()
        val state = (context as LabelyApplication).state
        state.appearance = Appearance.Dark
        assertEquals("dark", context.getSharedPreferences("labely", Context.MODE_PRIVATE).getString("appearance", null))
        assertEquals(Appearance.Dark, state.appearance)
        state.appearance = Appearance.System
    }
}
