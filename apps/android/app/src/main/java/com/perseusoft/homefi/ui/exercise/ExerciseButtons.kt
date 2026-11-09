package com.perseusoft.homefi.ui.exercise

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.ui.theme.Brand
import com.perseusoft.homefi.ui.theme.LocalMotion

private const val PRESSED_SCALE = 0.97f
private const val SELECTED_SCALE = 1.015f
private val BUTTON_HEIGHT = 60.dp
private val BUTTON_ELEVATION = 8.dp
private val BUTTON_RADIUS = 30.dp

/** A spring that squeezes [this] a little while pressed (only when the app's motion setting lets things move). */
@Composable
fun Modifier.pressScale(source: MutableInteractionSource): Modifier {
    val moves = LocalMotion.current.moves
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(
        if (moves && pressed) PRESSED_SCALE else 1f,
        spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMedium),
        label = "press",
    )
    return graphicsLayer { scaleX = scale; scaleY = scale }
}

/** A slight spring lift on the selected card or chip. */
@Composable
fun Modifier.selectScale(selected: Boolean): Modifier {
    val moves = LocalMotion.current.moves
    val scale by animateFloatAsState(
        if (moves && selected) SELECTED_SCALE else 1f,
        spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "select",
    )
    return graphicsLayer { scaleX = scale; scaleY = scale }
}

/** The big primary action: a rounded, elevated button filled with the brand gradient (or [colors]). */
@Composable
fun GradientButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    colors: List<Color>? = null,
) {
    val source = remember { MutableInteractionSource() }
    val shape = RoundedCornerShape(BUTTON_RADIUS)
    val brush = if (colors != null) Brush.horizontalGradient(colors) else Brand.gradient
    Row(
        modifier
            .pressScale(source)
            .shadow(BUTTON_ELEVATION, shape, ambientColor = Brand.Teal, spotColor = Brand.Teal)
            .defaultMinSize(minHeight = BUTTON_HEIGHT)
            .background(brush, shape)
            .clickable(interactionSource = source, indication = null, role = Role.Button, onClick = onClick)
            .padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) Icon(icon, null, Modifier.size(24.dp), tint = Color.White)
        Text(
            text,
            color = Color.White,
            fontSize = 18.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(start = if (icon != null) 8.dp else 0.dp),
        )
    }
}

/** A small tinted pill ("128 BPM · 1:1"): [color] on a 15 % wash of itself. */
@Composable
fun TintedPill(text: String, color: Color, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier.background(color.copy(alpha = PILL_ALPHA), RoundedCornerShape(50)).padding(horizontal = 10.dp, vertical = 4.dp),
        color = color,
        style = MaterialTheme.typography.labelMedium,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
    )
}

private const val PILL_ALPHA = 0.16f
