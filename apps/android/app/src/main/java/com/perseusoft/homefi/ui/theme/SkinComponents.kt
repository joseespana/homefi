package com.perseusoft.homefi.ui.theme

import android.os.Build
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CardColors
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CardElevation
import androidx.compose.material3.ListItemColors
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.SingletonImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.sin

// The shared pieces that read LocalSkin, so every screen picks the skin up.

/**
 * The app's theme root: the skin's colours, type and shapes, its base colour and (immersive)
 * the album behind everything.
 */
@Composable
fun HomefiTheme(skin: Skin, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalSkin provides skin) {
        MaterialTheme(colorScheme = skin.colors, typography = skin.typography, shapes = skin.shapes) {
            // the Surface sets the content colour, so text and icons drawn outside a Scaffold
            // (the full-screen player, dialogs' content) aren't left black in a dark skin
            Surface(color = skin.base, contentColor = skin.colors.onBackground) {
                if (skin.backdrop) SkinBackdrop()
                content()
            }
        }
    }
}

/** A full-screen overlay's background: the base colour and the skin's backdrop. */
@Composable
fun SkinScreen(modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    val skin = LocalSkin.current
    Box(modifier.fillMaxSize().background(skin.base)) {
        if (skin.backdrop) SkinBackdrop()
        CompositionLocalProvider(LocalContentColor provides skin.colors.onBackground) { content() }
    }
}

/**
 * The album, blurred and darkened (lightened in the light variant). Android 12+ blurs it for
 * real; older versions get it decoded tiny and stretched, which blurs it too.
 */
@Composable
fun SkinBackdrop(modifier: Modifier = Modifier) {
    val skin = LocalSkin.current
    val context = LocalContext.current
    val cover = skin.cover
    Box(modifier.fillMaxSize()) {
        if (cover != null) {
            val request = remember(cover) { ImageRequest.Builder(context).data(cover).size(48, 48).build() }
            AsyncImage(
                model = request,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                filterQuality = FilterQuality.High,
                modifier = Modifier
                    .fillMaxSize()
                    .then(if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) Modifier.blur(32.dp) else Modifier),
            )
        } else {
            // nothing playing: the accent as a soft wash
            val wash = remember(skin) {
                Brush.verticalGradient(listOf(lerp(skin.base, skin.colors.primary, 0.35f), skin.base))
            }
            Box(Modifier.fillMaxSize().background(wash))
        }
        skin.backdropScrim?.let { Box(Modifier.fillMaxSize().background(it)) }
    }
}

/**
 * The album's colour: the cover decoded at 16x16 and its saturated pixels averaged.
 * Null while loading, without a cover, or for a grey one.
 */
@Composable
fun rememberCoverAccent(cover: Any?): Color? {
    val context = LocalContext.current
    var accent by remember { mutableStateOf<Color?>(null) }
    LaunchedEffect(cover) {
        if (cover == null) {
            accent = null
            return@LaunchedEffect
        }
        val request = ImageRequest.Builder(context).data(cover).size(16, 16).allowHardware(false).build()
        val image = (SingletonImageLoader.get(context).execute(request) as? SuccessResult)?.image
        accent = image?.let { withContext(Dispatchers.Default) { dominant(it.toBitmap()) } }
    }
    return accent
}

/**
 * How light the lower part of the cover is (0 black, 1 white): where the immersive
 * layout puts the title. Null while loading or without a cover.
 */
@Composable
fun rememberCoverLightness(cover: Any?): Float? {
    val context = LocalContext.current
    var lightness by remember { mutableStateOf<Float?>(null) }
    LaunchedEffect(cover) {
        if (cover == null) {
            lightness = null
            return@LaunchedEffect
        }
        val request = ImageRequest.Builder(context).data(cover).size(16, 16).allowHardware(false).build()
        val image = (SingletonImageLoader.get(context).execute(request) as? SuccessResult)?.image
        lightness = image?.let { withContext(Dispatchers.Default) { lowerLightness(it.toBitmap()) } }
    }
    return lightness
}

/** Mean luminance of the bottom 40% of the picture, its middle half (the rest is cropped on a phone). */
private fun lowerLightness(bitmap: android.graphics.Bitmap): Float {
    var sum = 0f
    var n = 0
    for (px in bitmap.width / 4 until bitmap.width * 3 / 4) {
        for (py in bitmap.height * 3 / 5 until bitmap.height) {
            sum += Color(bitmap.getPixel(px, py)).luminance()
            n++
        }
    }
    return if (n == 0) 0f else sum / n
}

private fun dominant(bitmap: android.graphics.Bitmap): Color? {
    val hsv = FloatArray(3)
    var x = 0.0
    var y = 0.0
    var s = 0.0
    var weight = 0.0
    for (px in 0 until bitmap.width) {
        for (py in 0 until bitmap.height) {
            android.graphics.Color.colorToHSV(bitmap.getPixel(px, py), hsv)
            if (hsv[2] < 0.15f) continue
            val w = (hsv[1] * hsv[2]).toDouble()
            val a = Math.toRadians(hsv[0].toDouble())
            x += cos(a) * w
            y += sin(a) * w
            s += hsv[1] * w
            weight += w
        }
    }
    if (weight < bitmap.width * bitmap.height * 0.04) return null
    val hue = ((Math.toDegrees(atan2(y, x)) + 360) % 360).toFloat()
    return Color.hsv(hue, (s / weight).toFloat().coerceIn(0f, 1f), 0.8f)
}

// ------------------------------------------------------------------ drawing

/** Raised (or [sunken]) 3-D edge. */
fun DrawScope.drawBevel(bevel: Bevel, sunken: Boolean = false) {
    val w = bevel.width.toPx()
    val (top, bottom) = if (sunken) bevel.dark to bevel.light else bevel.light to bevel.dark
    drawRect(top, Offset.Zero, Size(size.width, w))
    drawRect(top, Offset.Zero, Size(w, size.height))
    drawRect(bottom, Offset(0f, size.height - w), Size(size.width, w))
    drawRect(bottom, Offset(size.width - w, 0f), Size(w, size.height))
}

/** A bevel drawn over the content's edges. */
fun Modifier.bevel(bevel: Bevel?, sunken: Boolean = false): Modifier =
    if (bevel == null) this else drawWithContent {
        drawContent()
        drawBevel(bevel, sunken)
    }

/**
 * Paints [chrome] behind the content: its brush, brushed-metal lines, gloss, bevel and the
 * edge line ([edgeAtTop] for bars under the content). Built once per size.
 */
fun Modifier.chrome(chrome: Chrome, edgeAtTop: Boolean): Modifier = drawWithCache {
    val lines = if (chrome.lines) {
        Path().apply {
            var y = 0f
            val step = 2.dp.toPx()
            while (y < size.height) {
                moveTo(0f, y)
                lineTo(size.width, y)
                y += step
            }
        }
    } else {
        null
    }
    val hairline = Stroke(1f)
    val gloss = if (chrome.gloss) {
        Brush.verticalGradient(listOf(Color.White.copy(alpha = 0.32f), Color.White.copy(alpha = 0.06f)), endY = size.height / 2)
    } else {
        null
    }
    val edge = chrome.edgeWidth.toPx()
    onDrawBehind {
        drawRect(chrome.brush)
        lines?.let { drawPath(it, Color.White.copy(alpha = 0.16f), style = hairline) }
        gloss?.let { drawRect(it, size = Size(size.width, size.height / 2)) }
        chrome.bevel?.let { drawBevel(it) }
        chrome.edge?.let { drawRect(it, Offset(0f, if (edgeAtTop) 0f else size.height - edge), Size(size.width, edge)) }
    }
}

/** A skinned button face: gradient, gloss, bevel or border, clipped to [shape]. */
private fun Modifier.face(face: SkinFace, shape: Shape): Modifier = clip(shape).drawWithCache {
    val fill = Brush.verticalGradient(listOf(face.top, face.bottom))
    val gloss = if (face.gloss) {
        Brush.verticalGradient(listOf(Color.White.copy(alpha = 0.55f), Color.White.copy(alpha = 0.1f)), endY = size.height / 2)
    } else {
        null
    }
    val outline = face.border?.let { shape.createOutline(size, layoutDirection, this) }
    val stroke = Stroke(1.dp.toPx() * 2) // half of it is clipped away
    onDrawBehind {
        drawRect(fill)
        gloss?.let { drawRect(it, size = Size(size.width, size.height / 2)) }
        face.bevel?.let { drawBevel(it) }
        if (outline != null && face.border != null) drawOutline(outline, face.border, style = stroke)
    }
}

// ------------------------------------------------------------------ buttons and cards

/**
 * A button drawn by the skin (bevelled, glossy...), with Material's size, touch target,
 * ripple and semantics.
 */
@Composable
private fun SkinnedButton(
    face: SkinFace,
    onClick: () -> Unit,
    modifier: Modifier,
    enabled: Boolean,
    shape: Shape,
    contentPadding: PaddingValues,
    content: @Composable RowScope.() -> Unit,
) {
    Box(
        modifier
            .minimumInteractiveComponentSize()
            .defaultMinSize(ButtonDefaults.MinWidth, ButtonDefaults.MinHeight)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f }
            .face(face, shape)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(contentPadding),
        contentAlignment = Alignment.Center,
        propagateMinConstraints = true,
    ) {
        CompositionLocalProvider(
            LocalContentColor provides face.content,
            LocalTextStyle provides MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
        ) {
            Row(horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically, content = content)
        }
    }
}

// Drop-in replacements for Material's buttons and cards (same names, import these instead).
// A caller's own colors keep Material's drawing, with the skin's shape.

@Composable
fun Button(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    shape: Shape? = null,
    colors: ButtonColors? = null,
    border: BorderStroke? = null,
    contentPadding: PaddingValues = ButtonDefaults.ContentPadding,
    content: @Composable RowScope.() -> Unit,
) {
    val skin = LocalSkin.current
    val s = shape ?: skin.buttonShape ?: ButtonDefaults.shape
    val face = skin.button
    if (face != null && colors == null) {
        SkinnedButton(face, onClick, modifier, enabled, s, contentPadding, content)
    } else {
        androidx.compose.material3.Button(
            onClick = onClick, modifier = modifier, enabled = enabled, shape = s,
            colors = colors ?: ButtonDefaults.buttonColors(), border = border, contentPadding = contentPadding, content = content,
        )
    }
}

@Composable
fun FilledTonalButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    shape: Shape? = null,
    colors: ButtonColors? = null,
    contentPadding: PaddingValues = ButtonDefaults.ContentPadding,
    content: @Composable RowScope.() -> Unit,
) {
    val skin = LocalSkin.current
    val s = shape ?: skin.buttonShape ?: ButtonDefaults.filledTonalShape
    val face = skin.tonalButton
    if (face != null && colors == null) {
        SkinnedButton(face, onClick, modifier, enabled, s, contentPadding, content)
    } else {
        androidx.compose.material3.FilledTonalButton(
            onClick = onClick, modifier = modifier, enabled = enabled, shape = s,
            colors = colors ?: ButtonDefaults.filledTonalButtonColors(),
            border = skin.frame?.let { BorderStroke(1.dp, it) }, contentPadding = contentPadding, content = content,
        )
    }
}

@Composable
fun OutlinedButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    shape: Shape? = null,
    colors: ButtonColors? = null,
    border: BorderStroke? = null,
    contentPadding: PaddingValues = ButtonDefaults.ContentPadding,
    content: @Composable RowScope.() -> Unit,
) {
    val skin = LocalSkin.current
    val s = shape ?: skin.buttonShape ?: ButtonDefaults.outlinedShape
    val face = skin.outlinedButton
    if (face != null && colors == null) {
        SkinnedButton(face, onClick, modifier, enabled, s, contentPadding, content)
    } else {
        androidx.compose.material3.OutlinedButton(
            onClick = onClick, modifier = modifier, enabled = enabled, shape = s,
            colors = colors ?: ButtonDefaults.outlinedButtonColors(),
            border = border ?: ButtonDefaults.outlinedButtonBorder(enabled), contentPadding = contentPadding, content = content,
        )
    }
}

/** Material's card with the skin's frame (border, bevel), shadow and glass. */
@Composable
fun Card(
    modifier: Modifier = Modifier,
    shape: Shape = CardDefaults.shape,
    colors: CardColors = CardDefaults.cardColors(),
    elevation: CardElevation? = null,
    border: BorderStroke? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val skin = LocalSkin.current
    androidx.compose.material3.Card(
        modifier = modifier.bevel(skin.bevel),
        shape = shape,
        colors = colors.glass(skin),
        elevation = elevation ?: CardDefaults.cardElevation(defaultElevation = skin.cardShadow),
        border = border ?: skin.frame?.let { BorderStroke(1.dp, it) },
        content = content,
    )
}

@Composable
fun Card(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    shape: Shape = CardDefaults.shape,
    colors: CardColors = CardDefaults.cardColors(),
    elevation: CardElevation? = null,
    border: BorderStroke? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val skin = LocalSkin.current
    androidx.compose.material3.Card(
        onClick = onClick,
        modifier = modifier.bevel(skin.bevel),
        enabled = enabled,
        shape = shape,
        colors = colors.glass(skin),
        elevation = elevation ?: CardDefaults.cardElevation(defaultElevation = skin.cardShadow),
        border = border ?: skin.frame?.let { BorderStroke(1.dp, it) },
        content = content,
    )
}

/** Over the immersive backdrop cards let the album through. */
private fun CardColors.glass(skin: Skin): CardColors =
    if (!skin.glass) this else copy(containerColor = containerColor.copy(alpha = containerColor.alpha.coerceAtMost(0.5f)))

// ------------------------------------------------------------------ rows and labels

/**
 * Colours of a list row: the skin's stripes on odd rows, and the row playing filled with
 * its selection colour (or only its title tinted, see [currentTitleColor]).
 */
@Composable
fun skinRowColors(index: Int, current: Boolean): ListItemColors {
    val skin = LocalSkin.current
    val selection = skin.selection
    return when {
        current && selection != null -> ListItemDefaults.colors(
            containerColor = selection,
            headlineColor = skin.onSelection,
            supportingColor = skin.onSelection.copy(alpha = 0.85f),
            leadingIconColor = skin.onSelection,
            trailingIconColor = skin.onSelection,
            overlineColor = skin.onSelection,
        )
        skin.stripe != null && index % 2 == 1 -> ListItemDefaults.colors(containerColor = skin.stripe)
        else -> ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface)
    }
}

/** Title of a row: the accent while it plays, unless the skin fills the whole row. */
@Composable
@ReadOnlyComposable
fun currentTitleColor(current: Boolean): Color =
    if (current && LocalSkin.current.selection == null) MaterialTheme.colorScheme.primary else Color.Unspecified

/** Durations and counts in the skin's figures (monospace on the console). */
@Composable
@ReadOnlyComposable
fun TextStyle.numeric(): TextStyle = LocalSkin.current.numbers?.let { copy(fontFamily = it) } ?: this

/** A label as the skin writes it (small capitals on the console). */
@Composable
@ReadOnlyComposable
fun skinCaps(text: String): String = if (LocalSkin.current.capsLabels) text.uppercase() else text

/**
 * A section title: plain, in small capitals on the console, or on a small gradient title bar
 * on the amp.
 */
@Composable
fun SkinHeading(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    fontWeight: FontWeight? = FontWeight.Bold,
) {
    val skin = LocalSkin.current
    val bar = skin.headingBar
    when {
        bar != null -> Text(
            text.uppercase(),
            style = style.copy(fontSize = 13.sp, letterSpacing = 2.sp),
            fontWeight = FontWeight.Bold,
            color = bar.second,
            maxLines = 1,
            modifier = modifier
                .fillMaxWidth()
                .drawBehind { drawRect(bar.first) }
                .padding(horizontal = 8.dp, vertical = 3.dp),
        )
        skin.capsLabels -> Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
            color = if (color == Color.Unspecified) MaterialTheme.colorScheme.onSurfaceVariant else color,
            maxLines = 1,
            modifier = modifier,
        )
        else -> Text(text, style = style, fontWeight = fontWeight, color = color, modifier = modifier)
    }
}

/** Panel of the player (spectrum, meters): the skin's container, radius and frame. */
fun Modifier.skinPanel(skin: Skin, color: Color, shape: Shape, width: Dp = 1.dp): Modifier {
    val framed = skin.frame
    return clip(shape)
        .background(color)
        .then(if (framed != null) Modifier.drawBehind { drawRect(framed, style = Stroke(width.toPx() * 2)) } else Modifier)
        .bevel(skin.bevel)
}
