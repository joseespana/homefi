package com.perseusoft.homefi.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * HomeFi's AI: HomeFito with headphones and a sparkle, the same drawing as the desktop's
 * `ai` icon (24 × 24, 2 dp outline). Used for the assistant, reactions and suggestions.
 */
val AiIcon: ImageVector by lazy {
    val ink = SolidColor(Color.Black)
    fun ImageVector.Builder.outline(d: String, width: Float = 2f) = addPath(
        addPathNodes(d),
        stroke = ink,
        strokeLineWidth = width,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    )
    fun ImageVector.Builder.solid(d: String) = addPath(addPathNodes(d), fill = ink)
    ImageVector.Builder("HomeFi.Ai", 24.dp, 24.dp, 24f, 24f).apply {
        // the house-shaped head and the headband over it
        outline("M4.8 14.2 9 10.4l4.2 3.8v5.1a1.2 1.2 0 0 1-1.2 1.2H6a1.2 1.2 0 0 1-1.2-1.2z")
        outline("M2.2 16.2C2.2 3.4 15.8 3.4 15.8 16.2")
        // ear cups, eyes and smile
        solid("M2.1 14.6h0.2a1.1 1.1 0 0 1 1.1 1.1v2.2a1.1 1.1 0 0 1-1.1 1.1h-0.2a1.1 1.1 0 0 1-1.1-1.1v-2.2a1.1 1.1 0 0 1 1.1-1.1z")
        solid("M15.7 14.6h0.2a1.1 1.1 0 0 1 1.1 1.1v2.2a1.1 1.1 0 0 1-1.1 1.1h-0.2a1.1 1.1 0 0 1-1.1-1.1v-2.2a1.1 1.1 0 0 1 1.1-1.1z")
        solid("M6.75 15.9a0.85 0.85 0 1 0 1.7 0a0.85 0.85 0 1 0-1.7 0z")
        solid("M9.55 15.9a0.85 0.85 0 1 0 1.7 0a0.85 0.85 0 1 0-1.7 0z")
        outline("M7.6 18.1q1.4 1.1 2.8 0", width = 1.5f)
        // the sparkles
        addPath(
            addPathNodes("M19.4 1.6l.85 2.45 2.45.85-2.45.85-.85 2.45-.85-2.45-2.45-.85 2.45-.85z"),
            fill = ink,
            stroke = ink,
            strokeLineWidth = 1f,
            strokeLineJoin = StrokeJoin.Round,
        )
        outline("M21.4 9.4v3M19.9 10.9h3", width = 1.6f)
    }.build()
}
