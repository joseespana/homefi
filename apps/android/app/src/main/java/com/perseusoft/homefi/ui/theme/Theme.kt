package com.perseusoft.homefi.ui.theme

import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color

// The colour schemes live with the skins (Skin.kt): classic is today's HomeFi look.

object Brand {
    val Blue = Color(0xFF1D4ED8)
    val Teal = Color(0xFF0891B2)
    val Green = Color(0xFF10B981)
    val gradient = Brush.linearGradient(listOf(Blue, Teal, Green))
}
