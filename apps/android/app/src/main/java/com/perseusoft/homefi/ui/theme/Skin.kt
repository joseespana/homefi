package com.perseusoft.homefi.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.data.PlayerLayout

// Each player layout is also a skin of the whole app: a Material color scheme, type and
// shapes, plus the few things Material can't say (bevels, gradient bars, glass, stripes),
// read from LocalSkin by the shared pieces (bars, mini player, rows, buttons, cards).
// The desktop implements the same spec; keep the colours in step with it.

/** A 3-D edge: light on top / left, dark on bottom / right (swapped when sunken). */
@Immutable
class Bevel(val light: Color, val dark: Color, val width: Dp = 2.dp)

/** Face of a skinned button: a vertical gradient, an optional gloss and a frame. */
@Immutable
class SkinFace(
    val top: Color,
    val bottom: Color,
    val content: Color,
    val border: Color? = null,
    val gloss: Boolean = false,
    val bevel: Bevel? = null,
)

/** The app's chrome: top bar, navigation bar, mini player. */
@Immutable
class Chrome(
    val brush: Brush,
    val content: Color,
    val contentVariant: Color,
    /** Behind the selected navigation item; transparent draws none. */
    val indicator: Color,
    val onIndicator: Color,
    /** Light system-bar icons over it. */
    val dark: Boolean,
    /** Fine horizontal lines (brushed metal). */
    val lines: Boolean = false,
    /** A highlight on the upper half (glossy / reflective). */
    val gloss: Boolean = false,
    val bevel: Bevel? = null,
    /** Line where the chrome meets the content (bottom of the top bar, top of the others). */
    val edge: Brush? = null,
    val edgeWidth: Dp = 1.dp,
)

@Immutable
class Skin(
    val layout: PlayerLayout,
    val dark: Boolean,
    val colors: ColorScheme,
    val typography: Typography,
    val shapes: Shapes,
    /** Opaque fill behind everything (full-screen overlays paint it before their content). */
    val base: Color,
    val topBar: Chrome? = null,
    val navBar: Chrome? = null,
    val miniPlayer: Chrome? = null,
    /** 1 dp border of cards and panels. */
    val frame: Color? = null,
    /** Raised edge of cards and panels (instead of [frame]). */
    val bevel: Bevel? = null,
    /** Soft drop shadow under cards. */
    val cardShadow: Dp = 0.dp,
    /** Odd list rows. */
    val stripe: Color? = null,
    /** The row playing, filled; null only colours its title. */
    val selection: Color? = null,
    val onSelection: Color = Color.White,
    /** Durations, counts, times. */
    val numbers: FontFamily? = null,
    /** Section labels in small capitals. */
    val capsLabels: Boolean = false,
    /** Small gradient bars behind section titles (title, ink). */
    val headingBar: Pair<Brush, Color>? = null,
    /** List thumbnails as round record labels. */
    val roundCovers: Boolean = false,
    val coverRadiusScale: Float = 1f,
    val coverRadiusMax: Dp = 64.dp,
    /** The album playing, blurred, behind the whole app. */
    val backdrop: Boolean = false,
    val cover: Any? = null,
    val backdropScrim: Brush? = null,
    /** Translucent cards over the backdrop. */
    val glass: Boolean = false,
    val button: SkinFace? = null,
    val tonalButton: SkinFace? = null,
    val outlinedButton: SkinFace? = null,
    val buttonShape: Shape? = null,
    /** Behind quality badges, when they are framed. */
    val badgeBevel: Bevel? = null,
) {
    /** A cover's corner radius in this skin; huge radii (circles) stay circles. */
    fun coverRadius(radius: Dp): Dp = if (radius >= 100.dp) radius else (radius * coverRadiusScale).coerceAtMost(coverRadiusMax)
}

val LocalSkin = staticCompositionLocalOf { skinOf(PlayerLayout.CLASSIC, dark = false) }

/** Skins that ignore the light / dark setting: true always dark, false always light. */
fun PlayerLayout.fixedDark(): Boolean? = when (this) {
    PlayerLayout.STUDIO, PlayerLayout.RETRO_AMP -> true
    PlayerLayout.LUNA, PlayerLayout.ALUMINUM, PlayerLayout.WEB2000 -> false
    else -> null
}

/**
 * The skin of [layout]. [accent] (the album's colour) and [cover] only matter to the
 * immersive one.
 */
fun skinOf(layout: PlayerLayout, dark: Boolean, accent: Color? = null, cover: Any? = null): Skin = when (layout) {
    PlayerLayout.CLASSIC -> classic(dark)
    PlayerLayout.IMMERSIVE -> immersive(dark, accent, cover)
    PlayerLayout.VINYL -> vinyl(dark)
    PlayerLayout.STUDIO -> studio()
    PlayerLayout.RETRO_AMP -> retroAmp()
    PlayerLayout.LUNA -> luna()
    PlayerLayout.ALUMINUM -> aluminum()
    PlayerLayout.WEB2000 -> web2000()
}

// ------------------------------------------------------------------ helpers

private fun shapes(xs: Dp, s: Dp, m: Dp, l: Dp, xl: Dp) = Shapes(
    extraSmall = RoundedCornerShape(xs),
    small = RoundedCornerShape(s),
    medium = RoundedCornerShape(m),
    large = RoundedCornerShape(l),
    extraLarge = RoundedCornerShape(xl),
)

private val Square = shapes(0.dp, 0.dp, 0.dp, 0.dp, 0.dp)

/** Material's type scale with other families for headings, body and labels. */
private fun typography(
    heading: FontFamily? = null,
    body: FontFamily? = null,
    label: FontFamily? = null,
    headingWeight: FontWeight? = null,
    labelSpacing: TextUnit? = null,
): Typography {
    val t = Typography()
    fun TextStyle.h() = copy(fontFamily = heading ?: fontFamily, fontWeight = headingWeight ?: fontWeight)
    fun TextStyle.b() = copy(fontFamily = body ?: fontFamily)
    fun TextStyle.l() = copy(fontFamily = label ?: fontFamily, letterSpacing = labelSpacing ?: letterSpacing)
    return Typography(
        displayLarge = t.displayLarge.h(),
        displayMedium = t.displayMedium.h(),
        displaySmall = t.displaySmall.h(),
        headlineLarge = t.headlineLarge.h(),
        headlineMedium = t.headlineMedium.h(),
        headlineSmall = t.headlineSmall.h(),
        titleLarge = t.titleLarge.h(),
        titleMedium = t.titleMedium.h(),
        titleSmall = t.titleSmall.h(),
        bodyLarge = t.bodyLarge.b(),
        bodyMedium = t.bodyMedium.b(),
        bodySmall = t.bodySmall.b(),
        labelLarge = t.labelLarge.l(),
        labelMedium = t.labelMedium.l(),
        labelSmall = t.labelSmall.l(),
    )
}

private fun vertical(vararg stops: Pair<Float, Color>) = Brush.verticalGradient(colorStops = stops)

// ------------------------------------------------------------------ classic

private val ClassicLight = lightColorScheme(
    primary = Color(0xFF0E7490),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFCFF4FA),
    onPrimaryContainer = Color(0xFF0B1E3A),
    secondary = Color(0xFF047857),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFD1FAE5),
    onSecondaryContainer = Color(0xFF064E3B),
    tertiary = Brand.Blue,
    background = Color(0xFFF7FBFC),
    surface = Color(0xFFF7FBFC),
    surfaceContainer = Color(0xFFEDF4F6),
    surfaceContainerHigh = Color(0xFFE3EDF0),
)

private val ClassicDark = darkColorScheme(
    primary = Color(0xFF67E3F5),
    onPrimary = Color(0xFF042B33),
    primaryContainer = Color(0xFF0E5566),
    onPrimaryContainer = Color(0xFFCFF4FA),
    secondary = Color(0xFF6EE7B7),
    onSecondary = Color(0xFF053B2A),
    secondaryContainer = Color(0xFF065F46),
    onSecondaryContainer = Color(0xFFD1FAE5),
    tertiary = Color(0xFF93C5FD),
    background = Color(0xFF0A121D),
    surface = Color(0xFF0A121D),
    surfaceContainer = Color(0xFF131E2E),
    surfaceContainerHigh = Color(0xFF1B283B),
)

private val ClassicType = Typography()
private val ClassicShapes = Shapes()

/** Today's HomeFi look, unchanged. */
private fun classic(dark: Boolean): Skin {
    val colors = if (dark) ClassicDark else ClassicLight
    return Skin(PlayerLayout.CLASSIC, dark, colors, ClassicType, ClassicShapes, base = colors.background)
}

// ------------------------------------------------------------------ immersive

private val ImmersiveShapes = shapes(10.dp, 14.dp, 22.dp, 28.dp, 36.dp)

/**
 * The album, blurred, behind everything; glass surfaces tinted with its colour, large
 * radii, no borders. Dark: the cover darkened; light: lightened, white glass.
 */
private fun immersive(dark: Boolean, accent: Color?, cover: Any?): Skin {
    val a = accent ?: if (dark) Color(0xFF67E3F5) else Color(0xFF0E7490)
    // readable on the glass whatever the cover
    val tone = a.withValue(if (dark) 0.95f else 0.5f, if (dark) 0.25f..0.6f else 0.45f..0.9f)
    val colors = if (dark) {
        val base = Color(0xFF0B0D12)
        val tint = lerp(Color(0xFF15171D), a, 0.16f)
        darkColorScheme(
            primary = tone,
            onPrimary = Color(0xFF0B0D12),
            primaryContainer = lerp(Color(0xFF1A1C22), a, 0.42f),
            onPrimaryContainer = Color.White,
            secondary = lerp(tone, Color.White, 0.35f),
            onSecondary = Color(0xFF0B0D12),
            secondaryContainer = Color.White.copy(alpha = 0.14f),
            onSecondaryContainer = Color.White,
            tertiary = lerp(tone, Color.White, 0.2f),
            background = Color.Transparent,
            onBackground = Color(0xFFF4F5F8),
            surface = Color.Transparent,
            onSurface = Color(0xFFF4F5F8),
            surfaceVariant = Color.White.copy(alpha = 0.10f),
            onSurfaceVariant = Color(0xFFC9CCD4),
            surfaceContainerLowest = base.copy(alpha = 0.6f),
            surfaceContainerLow = tint.copy(alpha = 0.95f),
            surfaceContainer = tint.copy(alpha = 0.95f),
            surfaceContainerHigh = lerp(tint, Color.White, 0.06f).copy(alpha = 0.95f),
            surfaceContainerHighest = Color.White.copy(alpha = 0.16f),
            outline = Color.White.copy(alpha = 0.32f),
            outlineVariant = Color.White.copy(alpha = 0.12f),
        ) to base
    } else {
        val base = Color(0xFFEFF1F5)
        val tint = lerp(Color.White, a, 0.08f)
        lightColorScheme(
            primary = tone,
            onPrimary = Color.White,
            primaryContainer = lerp(Color.White, a, 0.25f),
            onPrimaryContainer = Color(0xFF121418),
            secondary = lerp(tone, Color.Black, 0.2f),
            onSecondary = Color.White,
            secondaryContainer = Color.White.copy(alpha = 0.65f),
            onSecondaryContainer = Color(0xFF121418),
            tertiary = tone,
            background = Color.Transparent,
            onBackground = Color(0xFF121418),
            surface = Color.Transparent,
            onSurface = Color(0xFF121418),
            surfaceVariant = Color.White.copy(alpha = 0.5f),
            onSurfaceVariant = Color(0xFF3C414B),
            surfaceContainerLowest = Color.White.copy(alpha = 0.6f),
            surfaceContainerLow = tint.copy(alpha = 0.96f),
            surfaceContainer = tint.copy(alpha = 0.96f),
            surfaceContainerHigh = tint.copy(alpha = 0.96f),
            surfaceContainerHighest = Color.White.copy(alpha = 0.6f),
            outline = Color.Black.copy(alpha = 0.3f),
            outlineVariant = Color.Black.copy(alpha = 0.10f),
        ) to base
    }
    val (scheme, base) = colors
    val glassBar = if (dark) Color(0xFF0B0D12).copy(alpha = 0.45f) else Color.White.copy(alpha = 0.55f)
    val ink = scheme.onSurface
    val nav = Chrome(
        brush = SolidColor(glassBar),
        content = ink,
        contentVariant = scheme.onSurfaceVariant,
        indicator = if (dark) Color.White.copy(alpha = 0.16f) else Color.White.copy(alpha = 0.85f),
        onIndicator = ink,
        dark = dark,
    )
    return Skin(
        PlayerLayout.IMMERSIVE, dark, scheme, ClassicType, ImmersiveShapes,
        base = base,
        topBar = nav.copyBrush(SolidColor(Color.Transparent)),
        navBar = nav,
        miniPlayer = nav,
        coverRadiusScale = 1.5f,
        coverRadiusMax = 28.dp,
        backdrop = true,
        cover = cover,
        backdropScrim = if (dark) {
            vertical(0f to Color.Black.copy(alpha = 0.55f), 0.5f to Color.Black.copy(alpha = 0.62f), 1f to Color.Black.copy(alpha = 0.78f))
        } else {
            vertical(0f to Color.White.copy(alpha = 0.62f), 0.5f to Color.White.copy(alpha = 0.68f), 1f to Color.White.copy(alpha = 0.8f))
        },
        glass = true,
    )
}

private fun Chrome.copyBrush(brush: Brush) =
    Chrome(brush, content, contentVariant, indicator, onIndicator, dark, lines, gloss, bevel, edge, edgeWidth)

/** The same hue with this brightness, its saturation kept in [saturation]. */
private fun Color.withValue(value: Float, saturation: ClosedFloatingPointRange<Float>): Color {
    val hsv = FloatArray(3)
    android.graphics.Color.RGBToHSV((red * 255).toInt(), (green * 255).toInt(), (blue * 255).toInt(), hsv)
    return Color.hsv(hsv[0], hsv[1].coerceIn(saturation.start, saturation.endInclusive), value)
}

// ------------------------------------------------------------------ vinyl

private val VinylType = typography(heading = FontFamily.Serif)
private val VinylShapes = shapes(6.dp, 10.dp, 14.dp, 20.dp, 28.dp)

/** Record shop: cream paper / dark walnut, serif headings, amber, round labels. */
private fun vinyl(dark: Boolean): Skin {
    val colors = if (dark) {
        darkColorScheme(
            primary = Color(0xFFC2793A),
            onPrimary = Color(0xFF1E130A),
            primaryContainer = Color(0xFF5A3A1E),
            onPrimaryContainer = Color(0xFFF6DDBF),
            secondary = Color(0xFFD8B98F),
            onSecondary = Color(0xFF2A1D14),
            secondaryContainer = Color(0xFF453222),
            onSecondaryContainer = Color(0xFFEBD3B2),
            tertiary = Color(0xFFE0A070),
            background = Color(0xFF2A1D14),
            onBackground = Color(0xFFF1E4D0),
            surface = Color(0xFF2A1D14),
            onSurface = Color(0xFFF1E4D0),
            surfaceVariant = Color(0xFF3E2D20),
            onSurfaceVariant = Color(0xFFC9B49A),
            surfaceContainerLowest = Color(0xFF22170F),
            surfaceContainerLow = Color(0xFF31231A),
            surfaceContainer = Color(0xFF36281D),
            surfaceContainerHigh = Color(0xFF3D2E22),
            surfaceContainerHighest = Color(0xFF463527),
            outline = Color(0xFF8A7560),
            outlineVariant = Color(0xFF4A3A2C),
        )
    } else {
        lightColorScheme(
            primary = Color(0xFF9C5A24),
            onPrimary = Color.White,
            primaryContainer = Color(0xFFEBCFAE),
            onPrimaryContainer = Color(0xFF3A2210),
            secondary = Color(0xFF6B4A2E),
            onSecondary = Color.White,
            secondaryContainer = Color(0xFFE6D5BC),
            onSecondaryContainer = Color(0xFF3A2A1C),
            tertiary = Color(0xFF8A3B2E),
            background = Color(0xFFF4ECDF),
            onBackground = Color(0xFF2A1D14),
            surface = Color(0xFFF4ECDF),
            onSurface = Color(0xFF2A1D14),
            surfaceVariant = Color(0xFFE9DCC7),
            onSurfaceVariant = Color(0xFF6E5A48),
            surfaceContainerLowest = Color(0xFFFBF6EE),
            surfaceContainerLow = Color(0xFFF0E6D6),
            surfaceContainer = Color(0xFFEBDFCC),
            surfaceContainerHigh = Color(0xFFE5D7C1),
            surfaceContainerHighest = Color(0xFFDECDB4),
            outline = Color(0xFF9C8670),
            outlineVariant = Color(0xFFD8C7AE),
        )
    }
    return Skin(
        PlayerLayout.VINYL, dark, colors, VinylType, VinylShapes,
        base = colors.background,
        roundCovers = true,
    )
}

// ------------------------------------------------------------------ studio

private val StudioType = typography(labelSpacing = 1.2.sp)
private val StudioShapes = shapes(2.dp, 4.dp, 4.dp, 6.dp, 8.dp)
private val StudioBorder = Color(0xFF2C3036)

/** Pro audio console: charcoal, thin borders, small radii, monospace numbers, LEDs. */
private fun studio(): Skin {
    val colors = darkColorScheme(
        primary = Color(0xFF3DDC84),
        onPrimary = Color(0xFF04210F),
        primaryContainer = Color(0xFF163824),
        onPrimaryContainer = Color(0xFF9FF0C2),
        secondary = Color(0xFFFFB02E),
        onSecondary = Color(0xFF2B1B00),
        secondaryContainer = Color(0xFF2A2410),
        onSecondaryContainer = Color(0xFFFFB02E),
        tertiary = Color(0xFFFFB02E),
        background = Color(0xFF16181B),
        onBackground = Color(0xFFE4E7EB),
        surface = Color(0xFF16181B),
        onSurface = Color(0xFFE4E7EB),
        surfaceVariant = Color(0xFF23272B),
        onSurfaceVariant = Color(0xFF9BA3AD),
        surfaceContainerLowest = Color(0xFF121417),
        surfaceContainerLow = Color(0xFF1B1E21),
        surfaceContainer = Color(0xFF1F2226),
        surfaceContainerHigh = Color(0xFF23272B),
        surfaceContainerHighest = Color(0xFF2A2E33),
        outline = Color(0xFF3A3F46),
        outlineVariant = StudioBorder,
        error = Color(0xFFFF5C5C),
    )
    val panel = Chrome(
        brush = SolidColor(Color(0xFF1F2226)),
        content = Color(0xFFE4E7EB),
        contentVariant = Color(0xFF9BA3AD),
        indicator = Color(0xFF163824),
        onIndicator = Color(0xFF3DDC84),
        dark = true,
        edge = SolidColor(StudioBorder),
    )
    return Skin(
        PlayerLayout.STUDIO, true, colors, StudioType, StudioShapes,
        base = colors.background,
        topBar = panel,
        navBar = panel,
        miniPlayer = panel,
        frame = StudioBorder,
        numbers = FontFamily.Monospace,
        capsLabels = true,
        coverRadiusMax = 4.dp,
        buttonShape = RoundedCornerShape(4.dp),
    )
}

// ------------------------------------------------------------------ retro amp

internal val AmpFace = Color(0xFF2E2E40)
internal val AmpLight = Color(0xFF5C5C78)
internal val AmpDark = Color(0xFF12121A)
internal val LcdGreen = Color(0xFF00E000)
internal val AmpTitle = Brush.horizontalGradient(listOf(Color(0xFF3A3A55), Color(0xFF8A7A3A), Color(0xFF3A3A55)))
private val AmpType = typography(heading = FontFamily.Monospace, body = FontFamily.Monospace, label = FontFamily.Monospace)

/** The 2000s amp: near-black bevelled panels, LCD green text, blue selection, square. */
private fun retroAmp(): Skin {
    val colors = darkColorScheme(
        primary = LcdGreen,
        onPrimary = Color.Black,
        primaryContainer = Color(0xFF0000C6),
        onPrimaryContainer = Color.White,
        secondary = Color(0xFFE8C860),
        onSecondary = Color.Black,
        secondaryContainer = Color.Black,
        onSecondaryContainer = LcdGreen,
        tertiary = Color(0xFF5C9CFF),
        background = Color(0xFF0F0F14),
        onBackground = LcdGreen,
        surface = Color.Black,
        onSurface = LcdGreen,
        surfaceVariant = Color(0xFF1A1A24),
        onSurfaceVariant = Color(0xFFA8A8C0),
        surfaceContainerLowest = Color.Black,
        surfaceContainerLow = Color(0xFF1A1A24),
        surfaceContainer = Color(0xFF22222E),
        surfaceContainerHigh = AmpFace,
        surfaceContainerHighest = Color(0xFF34344A),
        outline = AmpLight,
        outlineVariant = AmpFace,
        error = Color(0xFFFF4040),
    )
    val bevel = Bevel(AmpLight, AmpDark)
    val panel = Chrome(
        brush = Brush.verticalGradient(listOf(Color(0xFF34344A), Color(0xFF1E1E2C))),
        content = Color(0xFFCFCFE0),
        contentVariant = Color(0xFF8A8AA0),
        indicator = Color.Transparent,
        onIndicator = LcdGreen,
        dark = true,
        bevel = bevel,
        edge = AmpTitle,
        edgeWidth = 3.dp,
    )
    val gray = SkinFace(
        top = Color(0xFFC8C8D8),
        bottom = Color(0xFFA8A8B8),
        content = Color(0xFF1A1A26),
        bevel = Bevel(Color(0xFFF0F0FA), Color(0xFF55556A)),
    )
    return Skin(
        PlayerLayout.RETRO_AMP, true, colors, AmpType, Square,
        base = colors.background,
        topBar = panel,
        navBar = panel,
        miniPlayer = panel,
        bevel = bevel,
        selection = Color(0xFF0000C6),
        onSelection = Color.White,
        numbers = FontFamily.Monospace,
        headingBar = AmpTitle to Color(0xFFE8E0B0),
        coverRadiusScale = 0f,
        coverRadiusMax = 0.dp,
        button = gray,
        tonalButton = gray,
        outlinedButton = gray,
        buttonShape = RectangleShape,
        badgeBevel = Bevel(AmpDark, AmpLight, 1.dp),
    )
}

// ------------------------------------------------------------------ luna

private val LunaType = typography(heading = FontFamily.SansSerif, body = FontFamily.SansSerif, label = FontFamily.SansSerif)
private val LunaShapes = shapes(3.dp, 4.dp, 6.dp, 8.dp, 8.dp)
internal val LunaTitle = Brush.verticalGradient(listOf(Color(0xFF0A246A), Color(0xFF3A6EA5), Color(0xFF0054E3)))

/** The early-2000s desktop: beige, blue title bars, glossy green primary buttons. */
private fun luna(): Skin {
    val colors = lightColorScheme(
        primary = Color(0xFF316AC5),
        onPrimary = Color.White,
        primaryContainer = Color(0xFFD6E4F8),
        onPrimaryContainer = Color(0xFF0A246A),
        secondary = Color(0xFF2E8B2E),
        onSecondary = Color.White,
        secondaryContainer = Color(0xFFDCE7F7),
        onSecondaryContainer = Color(0xFF0A246A),
        tertiary = Color(0xFF0054E3),
        background = Color(0xFFECE9D8),
        onBackground = Color.Black,
        surface = Color.White,
        onSurface = Color.Black,
        surfaceVariant = Color(0xFFF1EFE2),
        onSurfaceVariant = Color(0xFF4B4B4B),
        surfaceContainerLowest = Color.White,
        surfaceContainerLow = Color(0xFFF6F5EE),
        surfaceContainer = Color(0xFFF1EFE2),
        surfaceContainerHigh = Color(0xFFF4F2E6),
        surfaceContainerHighest = Color(0xFFE6E2CC),
        outline = Color(0xFF7F9DB9),
        outlineVariant = Color(0xFFD4D0C8),
    )
    val bar = Chrome(
        brush = LunaTitle,
        content = Color.White,
        contentVariant = Color(0xFFD6E4F8),
        indicator = Color(0xFF0A246A).copy(alpha = 0.55f),
        onIndicator = Color.White,
        dark = true,
        gloss = true,
    )
    val pane = Chrome(
        brush = Brush.verticalGradient(listOf(Color(0xFFF2F6FD), Color(0xFFC6D3F7))),
        content = Color(0xFF0A246A),
        contentVariant = Color(0xFF3A4F80),
        indicator = Color.Transparent,
        onIndicator = Color(0xFF0A246A),
        dark = false,
        edge = SolidColor(Color(0xFF7F9DB9)),
    )
    return Skin(
        PlayerLayout.LUNA, false, colors, LunaType, LunaShapes,
        base = colors.background,
        topBar = bar,
        navBar = bar,
        miniPlayer = pane,
        frame = Color(0xFF7F9DB9),
        selection = Color(0xFF316AC5),
        onSelection = Color.White,
        coverRadiusMax = 4.dp,
        button = SkinFace(Color(0xFF5EC35E), Color(0xFF2E8B2E), Color.White, border = Color(0xFF1E5E1E), gloss = true),
        tonalButton = SkinFace(Color.White, Color(0xFFD9D4C6), Color.Black, border = Color(0xFF003C74), gloss = true),
        outlinedButton = SkinFace(Color.White, Color(0xFFECEBE6), Color.Black, border = Color(0xFF003C74)),
        buttonShape = RoundedCornerShape(8.dp),
    )
}

// ------------------------------------------------------------------ aluminum

private val AluminumShapes = shapes(4.dp, 6.dp, 8.dp, 10.dp, 12.dp)
internal val Metal = Brush.verticalGradient(listOf(Color(0xFFC9C9C9), Color(0xFFA9A9A9)))

/** Brushed metal chrome, aqua pills, source list, striped rows. */
private fun aluminum(): Skin {
    val colors = lightColorScheme(
        primary = Color(0xFF3875D7),
        onPrimary = Color.White,
        primaryContainer = Color(0xFFD4E2F8),
        onPrimaryContainer = Color(0xFF0D2C66),
        secondary = Color(0xFF4F5F75),
        onSecondary = Color.White,
        secondaryContainer = Color(0xFFDFE5EE),
        onSecondaryContainer = Color(0xFF2B3440),
        tertiary = Color(0xFF3875D7),
        background = Color(0xFFDFE5EE),
        onBackground = Color(0xFF1E1E1E),
        surface = Color.White,
        onSurface = Color(0xFF1E1E1E),
        surfaceVariant = Color(0xFFE9EDF2),
        onSurfaceVariant = Color(0xFF555A60),
        surfaceContainerLowest = Color.White,
        surfaceContainerLow = Color(0xFFF2F4F7),
        surfaceContainer = Color(0xFFE9EDF2),
        surfaceContainerHigh = Color(0xFFF2F4F7),
        surfaceContainerHighest = Color(0xFFD9DFE7),
        outline = Color(0xFF8C8C8C),
        outlineVariant = Color(0xFFC9CED6),
    )
    val metal = Chrome(
        brush = Metal,
        content = Color(0xFF1E1E1E),
        contentVariant = Color(0xFF3A3A3A),
        indicator = Color.White.copy(alpha = 0.6f),
        onIndicator = Color(0xFF0F3E9A),
        dark = false,
        lines = true,
        edge = SolidColor(Color(0xFF7A7A7A)),
    )
    return Skin(
        PlayerLayout.ALUMINUM, false, colors, ClassicType, AluminumShapes,
        base = colors.background,
        topBar = metal,
        navBar = metal,
        miniPlayer = metal,
        frame = Color(0xFFA9A9A9),
        stripe = Color(0xFFEDF3FE),
        selection = Color(0xFF3875D7),
        onSelection = Color.White,
        coverRadiusMax = 4.dp,
        button = SkinFace(Color(0xFF8CC4FF), Color(0xFF2C74D9), Color.White, border = Color(0xFF1E4F9A), gloss = true),
        tonalButton = SkinFace(Color.White, Color(0xFFCFCFCF), Color(0xFF1E1E1E), border = Color(0xFF8C8C8C), gloss = true),
        outlinedButton = SkinFace(Color.White, Color(0xFFE6E6E6), Color(0xFF1E1E1E), border = Color(0xFF8C8C8C)),
        buttonShape = RoundedCornerShape(50),
    )
}

// ------------------------------------------------------------------ web 2000

private val WebType = typography(headingWeight = FontWeight.Bold)
private val WebShapes = shapes(6.dp, 8.dp, 10.dp, 10.dp, 12.dp)
internal val WebOrange = Color(0xFFFF7A00)
internal val WebLime = Color(0xFF8DC63F)

/** Glossy Web 2.0: white, orange and lime, reflective header bars, soft shadows. */
private fun web2000(): Skin {
    val colors = lightColorScheme(
        primary = Color(0xFFB85300),
        onPrimary = Color.White,
        primaryContainer = Color(0xFFFFE6CC),
        onPrimaryContainer = Color(0xFF5A2800),
        secondary = Color(0xFF4F7A18),
        onSecondary = Color.White,
        secondaryContainer = Color(0xFFE4F1D2),
        onSecondaryContainer = Color(0xFF2F4F0F),
        tertiary = WebLime,
        background = Color(0xFFF4F4F4),
        onBackground = Color(0xFF222222),
        surface = Color.White,
        onSurface = Color(0xFF222222),
        surfaceVariant = Color(0xFFF2F2F2),
        onSurfaceVariant = Color(0xFF5E5E5E),
        surfaceContainerLowest = Color.White,
        surfaceContainerLow = Color(0xFFFAFAFA),
        surfaceContainer = Color(0xFFF2F2F2),
        surfaceContainerHigh = Color.White,
        surfaceContainerHighest = Color(0xFFE9E9E9),
        outline = Color(0xFFB5B5B5),
        outlineVariant = Color(0xFFE0E0E0),
    )
    // the charcoal of the player's header, reflective
    val header = Chrome(
        brush = vertical(0f to Color(0xFF606064), 0.5f to Color(0xFF424246), 0.5f to Color(0xFF333336), 1f to Color(0xFF3C3C3F)),
        content = Color(0xFFF2F2F2),
        contentVariant = Color(0xFFBDBDBD),
        indicator = WebOrange,
        onIndicator = Color.White,
        dark = true,
        gloss = true,
    )
    val light = Chrome(
        brush = vertical(0f to Color.White, 0.5f to Color(0xFFF4F4F4), 0.5f to Color(0xFFE8E8E8), 1f to Color(0xFFF2F2F2)),
        content = Color(0xFF222222),
        contentVariant = Color(0xFF5E5E5E),
        indicator = Color(0xFFFFE6CC),
        onIndicator = Color(0xFFB85300),
        dark = false,
        edge = SolidColor(Color(0xFFD0D0D0)),
    )
    return Skin(
        PlayerLayout.WEB2000, false, colors, WebType, WebShapes,
        base = colors.background,
        topBar = header,
        navBar = header,
        miniPlayer = light,
        frame = Color(0xFFE0E0E0),
        cardShadow = 3.dp,
        coverRadiusMax = 8.dp,
        button = SkinFace(Color(0xFFFFA040), Color(0xFFE06A00), Color.White, border = Color(0xFFC85A00), gloss = true),
        tonalButton = SkinFace(Color(0xFFB5E07A), WebLime, Color(0xFF1E3300), border = Color(0xFF6E9E2C), gloss = true),
        outlinedButton = SkinFace(Color.White, Color(0xFFE4E4E4), Color(0xFF333333), border = Color(0xFFB5B5B5), gloss = true),
        buttonShape = RoundedCornerShape(10.dp),
    )
}
