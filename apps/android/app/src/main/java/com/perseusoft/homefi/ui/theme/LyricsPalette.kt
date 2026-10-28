package com.perseusoft.homefi.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import com.perseusoft.homefi.data.PlayerLayout

/**
 * Colours of the lyrics over the player's background: [ink] for the line being sung (and
 * plain text), [dim] for the others, [fill] → [fillEnd] for the karaoke sweep. [panel]
 * is drawn behind the lines where the background alone isn't calm enough (the immersive
 * cover, the amp's LCD), and [label] is the small print around it.
 */
@Immutable
class LyricsPalette(
    val ink: Color,
    val dim: Color,
    val fill: Color,
    val fillEnd: Color,
    val panel: Color? = null,
    /** The amp's LCD: a sunken, bevelled screen. */
    val lcd: Boolean = false,
    /** Small print outside the [panel] (title, credit, times); [dim] when there is no panel. */
    val label: Color = dim,
) {
    /** Text on [fill] (the selected mode, the play button). */
    val onFill: Color get() = if (fill.luminance() > 0.45f) Color(0xFF111111) else Color.White
}

/**
 * The lyrics colours of [layout]'s player. The retro players paint their own background
 * whatever the app's skin is, so they get fixed colours; the others follow the theme.
 */
@Composable
fun lyricsPalette(layout: PlayerLayout): LyricsPalette {
    val c = MaterialTheme.colorScheme
    return when (layout) {
        PlayerLayout.RETRO_AMP -> LyricsPalette(
            ink = LcdGreen,
            dim = Color(0xFF0B5E0B),
            fill = LcdGreen,
            fillEnd = Color(0xFFB4FFB4),
            panel = Color(0xFF020602),
            lcd = true,
            label = Color(0xFFA8A8C0),
        )
        // white on the blue desktop, the sweep in the taskbar's warm yellow
        PlayerLayout.LUNA -> LyricsPalette(
            ink = Color.White,
            dim = Color.White.copy(alpha = 0.55f),
            fill = Color(0xFFFFD24A),
            fillEnd = Color(0xFFFFA726),
        )
        PlayerLayout.ALUMINUM -> LyricsPalette(
            ink = Color(0xFF222222),
            dim = Color(0xFF222222).copy(alpha = 0.45f),
            fill = Color(0xFF3875D7),
            fillEnd = Color(0xFF6FA3F0),
        )
        PlayerLayout.WEB2000 -> LyricsPalette(
            ink = Color(0xFFEDEDED),
            dim = Color(0xFFEDEDED).copy(alpha = 0.5f),
            fill = WebOrange,
            fillEnd = WebLime,
        )
        else -> {
            LyricsPalette(
                ink = c.onSurface,
                dim = c.onSurface.copy(alpha = 0.45f),
                fill = c.primary,
                fillEnd = lerp(c.primary, c.tertiary, 0.6f),
                // the skin's opaque base, see-through: the cover stays behind, the words stay readable
                panel = if (layout == PlayerLayout.IMMERSIVE) LocalSkin.current.base.copy(alpha = 0.6f) else null,
            )
        }
    }
}
