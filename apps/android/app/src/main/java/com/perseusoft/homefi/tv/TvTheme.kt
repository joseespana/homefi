package com.perseusoft.homefi.tv

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.theme.HomefiTheme
import com.perseusoft.homefi.ui.theme.ProvideMotion
import com.perseusoft.homefi.ui.theme.Skin
import com.perseusoft.homefi.ui.theme.skinOf
import androidx.tv.material3.MaterialTheme as TvMaterialTheme
import androidx.tv.material3.darkColorScheme as tvDarkColorScheme

/** Sizes of the TV UI in dp (a 1080p TV is 960 x 540 dp, so 1 dp = 2 px). Spec: `docs/TV_UI.md` section 2. */
object TvDimens {
    /** Overscan-safe margins (`tv.safeMargins`). */
    val safeSide = 48.dp
    val safeVertical = 27.dp
    val gap = 16.dp
    val cardRadius = 12.dp
    val panelRadius = 14.dp

    /** Width of the collapsed rail with its margin, and where the content starts. */
    val rail = 84.dp
    val contentStart = 132.dp
    val drawerOpen = 320.dp
    val railIcon = 36.dp
    val itemHeight = 36.dp

    /** Smallest focus target (pills, rows are 48 dp). */
    val pillHeight = 36.dp
    val coverCard = 115.dp
}

/** Type scale in sp: 1 sp = 2 px at 1080p, nothing under 12 sp. Spec: `docs/TV_UI.md` section 2. */
object TvType {
    val display = TextStyle(fontSize = 36.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.4).sp)
    val title = TextStyle(fontSize = 32.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.3).sp)
    val song = TextStyle(fontSize = 30.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.3).sp)
    val section = TextStyle(fontSize = 20.sp, fontWeight = FontWeight.Bold)
    val body = TextStyle(fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    val cardTitle = TextStyle(fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
    val secondary = TextStyle(fontSize = 12.sp, fontWeight = FontWeight.Medium)
    val caps = TextStyle(fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 2.sp)
}

/** The TV palette (dark): background, ink and the accent gradient (the skin's, or HomeFi's blue to teal). */
@Immutable
class TvLook(
    val accent: Color,
    val accent2: Color,
    val bg: Color = Color(0xFF060810),
    val bg2: Color = Color(0xFF0B1020),
    val ink: Color = Color(0xFFF4F7FB),
    val ink2: Color = Color(0xFFA9B6CB),
    val ink3: Color = Color(0xFF6F7D94),
    val ok: Color = Color(0xFF34D399),
    val warn: Color = Color(0xFFFBBF24),
    val error: Color = Color(0xFFF87171),
) {
    val gradient: Brush = Brush.horizontalGradient(listOf(accent, accent2))
    val glassFill: Brush = Brush.linearGradient(listOf(Color.White.copy(alpha = 0.09f), Color.White.copy(alpha = 0.03f)))
    val glassLine: Color = Color.White.copy(alpha = 0.10f)

    /** Text on the accent gradient. */
    val onAccent: Color = Color(0xFF04101C)

    /** Text on the white focus fill. */
    val onFocus: Color = Color(0xFF0A1020)
}

val LocalTvLook = staticCompositionLocalOf { tvLookOf(skinOf(PlayerLayout.CLASSIC, dark = true)) }

/** Keep content inside the overscan-safe margins. */
val LocalTvSafeMargins = staticCompositionLocalOf { true }

/** The skins that stay readable on the TV's always-dark glass; the others fall back to the classic one. */
internal fun tvSkinLayout(layout: PlayerLayout): PlayerLayout = when (layout) {
    PlayerLayout.VINYL, PlayerLayout.STUDIO -> layout
    else -> PlayerLayout.CLASSIC
}

/** HomeFi's blue and teal for the classic skin; any other skin brings its own accent. */
internal fun tvLookOf(skin: Skin): TvLook =
    if (skin.layout == PlayerLayout.CLASSIC) TvLook(Color(0xFF4F8CFF), Color(0xFF22D3C5))
    else TvLook(skin.colors.primary, skin.colors.tertiary)

/**
 * Maps the app's [Skin] into both Material themes (the shared draw-only pieces read `LocalSkin`
 * and Material 3, the TV widgets read tv-material), dark always, and applies the TV text size.
 */
@Composable
fun TvTheme(content: @Composable () -> Unit) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val layout = tvSkinLayout(settings.playerLayout)
    val skin = remember(layout) { skinOf(layout, dark = true) }
    val look = remember(skin) { tvLookOf(skin) }
    val density = LocalDensity.current
    val scaled = remember(density, settings.tv.textScale) { Density(density.density, density.fontScale * settings.tv.textScale) }
    CompositionLocalProvider(
        LocalDensity provides scaled,
        LocalTvLook provides look,
        LocalTvSafeMargins provides settings.tv.safeMargins,
    ) {
        HomefiTheme(skin) {
            ProvideMotion(settings.motion, layout) {
                TvMaterialTheme(
                    colorScheme = tvDarkColorScheme(
                        primary = look.accent,
                        onPrimary = look.onAccent,
                        secondary = look.accent2,
                        background = look.bg,
                        onBackground = look.ink,
                        surface = look.bg2,
                        onSurface = look.ink,
                        surfaceVariant = look.bg2,
                        onSurfaceVariant = look.ink2,
                        border = look.glassLine,
                        error = look.error,
                    ),
                    content = content,
                )
            }
        }
    }
}

/** The safe-area padding of a screen (zero when the user turned the margins off). */
@Composable
fun tvSafePadding(): PaddingValues =
    if (LocalTvSafeMargins.current) PaddingValues(horizontal = TvDimens.safeSide, vertical = TvDimens.safeVertical) else PaddingValues()

/**
 * The calm backdrop of every screen: the dark gradient with two soft lights (the accent top right,
 * the second accent bottom left). [tint] replaces the first light (the player uses the cover's colour).
 */
@Composable
fun TvBackdrop(modifier: Modifier = Modifier, tint: Color? = null, content: @Composable () -> Unit = {}) {
    val look = LocalTvLook.current
    val motion = com.perseusoft.homefi.ui.theme.LocalMotion.current
    val light by androidx.compose.animation.animateColorAsState(tint ?: look.accent, motion.tween(BACKDROP_TINT_MS), label = "tv-backdrop-tint")
    Box(
        modifier
            .background(Brush.verticalGradient(listOf(look.bg2, look.bg)))
            .drawBehind {
                val big = size.maxDimension
                drawRect(
                    Brush.radialGradient(listOf(light.copy(alpha = 0.30f), Color.Transparent), Offset(size.width * 0.78f, size.height * 0.08f), big * 0.5f),
                )
                drawRect(
                    Brush.radialGradient(listOf(look.accent2.copy(alpha = 0.16f), Color.Transparent), Offset(size.width * 0.08f, size.height), big * 0.45f),
                )
                // a vignette keeps the corners calm on a dim room
                drawRect(
                    Brush.radialGradient(listOf(Color.Transparent, Color(0xB302030A)), Offset(size.width / 2, size.height * 0.45f), big * 0.75f),
                )
            },
    ) { content() }
}

/** [safe] padding applied around [content]. */
@Composable
fun Modifier.tvSafe(): Modifier = padding(tvSafePadding())

/** The backdrop takes a new cover's colour over this long. */
private const val BACKDROP_TINT_MS = 700
