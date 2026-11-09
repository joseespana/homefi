package com.perseusoft.homefi.ui.dj

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.material3.MaterialTheme

/**
 * "Homifito colours": the whole DJ board in his palette (mint deck A, pink deck B, navy waveforms), in a
 * light and a dark variant. Same values as the desktop's `.dj.hlook`. Null in [LocalDjLook] = the skin's own.
 */
@Immutable
class DjLook(val deckA: Color, val deckB: Color, val waveBackground: Color, val playhead: Color) {
    companion object {
        private val DARK = DjLook(Color(0xFF10B981), Color(0xFFF472B6), Color(0xFF0B1E3A), Color(0xFFFDE68A))
        private val LIGHT = DjLook(Color(0xFF059669), Color(0xFFDB2777), Color(0xFFE3F6EF), Color(0xFF0B1E3A))
        fun of(dark: Boolean) = if (dark) DARK else LIGHT
    }
}

val LocalDjLook = compositionLocalOf<DjLook?> { null }

/** [DjLook] for the current surface (dark or light by its luminance). */
@Composable
fun homifitoLook(): DjLook = DjLook.of(MaterialTheme.colorScheme.background.luminance() < 0.5f)
