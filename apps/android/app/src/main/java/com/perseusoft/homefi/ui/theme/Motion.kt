package com.perseusoft.homefi.ui.theme

import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.grid.LazyGridItemScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.data.Motion
import com.perseusoft.homefi.data.PlayerLayout
import kotlin.math.ceil
import kotlin.math.roundToInt

// The app's transitions in one place, so the "motion" setting (auto, full, subtle, off) and
// the skin's era decide every one of them; the desktop follows the same rules. Everything
// moves with graphicsLayer (alpha, translation, scale) or layout placement, never by
// re-measuring, so a low-end DAP keeps up.

/**
 * How a skin moves, as on the desktop: today's springs with a slight bounce, Aluminum's
 * smooth eases (0.8×, no bounce), Luna's and Web 2000's plain quick ones (0.6×), and
 * Retro Amp's chunky few-frame steps (0.55×).
 */
enum class MotionStyle(val pace: Float) { MODERN(1f), SMOOTH(0.8f), PLAIN(0.6f), STEPPED(0.55f) }

fun PlayerLayout.motionStyle(): MotionStyle = when (this) {
    PlayerLayout.RETRO_AMP -> MotionStyle.STEPPED
    PlayerLayout.LUNA, PlayerLayout.WEB2000 -> MotionStyle.PLAIN
    PlayerLayout.ALUMINUM -> MotionStyle.SMOOTH
    else -> MotionStyle.MODERN
}

/** AUTO resolved: FULL, or SUBTLE when the system asks for less motion. */
fun Motion.resolve(systemReduced: Boolean): Motion = when (this) {
    Motion.AUTO -> if (systemReduced) Motion.SUBTLE else Motion.FULL
    else -> this
}

/** The same transition with another size animation (`using` only exists inside a transitionSpec). */
private fun ContentTransform.sized(size: SizeTransform?) = ContentTransform(targetContentEnter, initialContentExit, targetContentZIndex, size)

/** Jumps in [steps] even steps, like a 90s window that animates in a handful of frames. */
private class Steps(private val steps: Int) : Easing {
    override fun transform(fraction: Float): Float = (ceil(fraction * steps) / steps).coerceAtMost(1f)
}

/**
 * Specs and transitions for one resolved [level] (FULL, SUBTLE or OFF) and [style]. SUBTLE
 * keeps short fades only (nothing slides or grows); OFF returns snaps and `None`
 * transitions, so content swaps at once with nothing left running.
 */
@Immutable
class MotionScheme(val level: Motion, val style: MotionStyle) {
    val enabled: Boolean get() = level != Motion.OFF

    /** Things may slide, grow and bounce (FULL); SUBTLE only fades. */
    val moves: Boolean get() = level == Motion.FULL || level == Motion.AUTO

    /** The cover flying between the mini player and the full player. */
    val sharedCover: Boolean get() = moves && style != MotionStyle.STEPPED

    /** The first rows of a list arriving one after another. */
    val cascade: Boolean get() = moves && style != MotionStyle.STEPPED

    /** Duration multiplier: period skins are snappier, SUBTLE's fades a bit quicker. */
    private val pace: Float = style.pace * (if (moves) 1f else 0.8f)

    /** How far screens travel, as a fraction of their width or height. */
    private val travel: Float = when {
        !moves || style == MotionStyle.STEPPED -> 0f
        style == MotionStyle.MODERN -> 0.15f
        else -> 0.08f
    }

    private val bouncy: Boolean get() = moves && style == MotionStyle.MODERN

    private val easing: Easing = when (style) {
        MotionStyle.MODERN, MotionStyle.SMOOTH -> FastOutSlowInEasing
        MotionStyle.PLAIN -> LinearOutSlowInEasing
        MotionStyle.STEPPED -> Steps(4)
    }

    fun ms(base: Int): Int = (base * pace).roundToInt()

    /** A timed animation (fades, colours) at this scheme's pace. */
    fun <T> tween(base: Int = 300, delay: Int = 0): FiniteAnimationSpec<T> =
        if (!enabled) snap() else androidx.compose.animation.core.tween(ms(base), ms(delay), easing)

    /** Movement: a spring with a slight bounce in the modern skins, a short tween in the period ones. */
    fun <T> move(visibilityThreshold: T? = null): FiniteAnimationSpec<T> = when {
        !enabled -> snap()
        style == MotionStyle.MODERN -> spring(
            dampingRatio = if (bouncy) 0.82f else 1f,
            stiffness = if (bouncy) 380f else 700f,
            visibilityThreshold = visibilityThreshold,
        )
        else -> androidx.compose.animation.core.tween(ms(280), easing = easing)
    }

    fun color(): AnimationSpec<Color> = tween(220)

    private val offsetMove get() = move(IntOffset.VisibilityThreshold)

    /** Material's fade-through: the old content fades quickly, then the new one fades (and grows) in. */
    private fun fadeThroughIn(): EnterTransition =
        fadeIn(tween(210, delay = 90)) +
            if (bouncy) scaleIn(tween(300), initialScale = 0.94f) else EnterTransition.None

    private fun fadeThroughOut(): ExitTransition = fadeOut(tween(90))

    /** Switching between destinations; [forward] slides in from the end, back from the start (shared X axis). */
    fun screens(forward: Boolean): ContentTransform {
        if (!enabled) return NONE
        val dir = if (forward) 1 else -1
        val enter = fadeThroughIn().takeIf { travel == 0f }
            ?: (fadeIn(tween(210, delay = 60)) + slideInHorizontally(offsetMove) { (it * travel).roundToInt() * dir })
        val exit = if (travel == 0f) {
            fadeThroughOut()
        } else {
            fadeOut(tween(120)) + slideOutHorizontally(offsetMove) { -(it * travel).roundToInt() * dir }
        }
        return (enter togetherWith exit).sized(null)
    }

    /** Fade-through that also eases the size between the two (a hint that gets longer). */
    fun swap(): ContentTransform = if (!enabled) NONE else (fadeIn(tween(200, delay = 80)) togetherWith fadeOut(tween(80)))
        .sized(if (moves) SizeTransform(clip = false) { _, _ -> move(IntSize.VisibilityThreshold) } else null)

    /** Same place, other content (a login replaced by the screen, one player layout by another). */
    fun fadeThrough(): ContentTransform = if (!enabled) NONE else (fadeThroughIn() togetherWith fadeThroughOut()).sized(null)

    /** Something coming up from below ([up]), or going back down (the lyrics over the player). */
    fun vertical(up: Boolean): ContentTransform {
        if (!enabled) return NONE
        if (travel == 0f) return fadeThrough()
        val dir = if (up) 1 else -1
        return (
            (fadeIn(tween(220, delay = 60)) + slideInVertically(offsetMove) { (it * travel).roundToInt() * dir }) togetherWith
                (fadeOut(tween(120)) + slideOutVertically(offsetMove) { -(it * travel).roundToInt() * dir })
            ).sized(null)
    }

    /** A new song: the old title / cover leaves the way the queue moved, the new one comes from the other side. */
    fun track(forward: Boolean): ContentTransform {
        if (!enabled) return NONE
        if (!moves || style == MotionStyle.STEPPED) return (fadeIn(tween(160)) togetherWith fadeOut(tween(100))).sized(SizeTransform(clip = false))
        val dir = if (forward) 1 else -1
        val shift = if (style == MotionStyle.MODERN) 0.25f else 0.12f
        return (
            (fadeIn(tween(240, delay = 40)) + slideInHorizontally(offsetMove) { (it * shift).roundToInt() * dir }) togetherWith
                (fadeOut(tween(140)) + slideOutHorizontally(offsetMove) { -(it * shift).roundToInt() * dir })
            ).sized(SizeTransform(clip = false))
    }

    /** A small change in place (a line of text, a counter): a quick vertical tick. */
    fun ticker(): ContentTransform {
        if (!enabled) return NONE
        if (!moves || style == MotionStyle.STEPPED) return (fadeIn(tween(120)) togetherWith fadeOut(tween(80))).sized(SizeTransform(clip = false))
        return (
            (fadeIn(tween(200, delay = 40)) + slideInVertically(offsetMove) { it / 2 }) togetherWith
                (fadeOut(tween(120)) + slideOutVertically(offsetMove) { -it / 2 })
            ).sized(SizeTransform(clip = false))
    }

    /** A full-screen panel (the player, a guest's room) coming up from the mini player. */
    fun panelEnter(): EnterTransition {
        if (!enabled) return EnterTransition.None
        if (!moves) return fadeIn(tween(200))
        val rise = if (style == MotionStyle.MODERN) 0.35f else 1f
        return slideInVertically(offsetMove) { (it * rise).roundToInt() } +
            fadeIn(tween(if (style == MotionStyle.MODERN) 220 else 120)) +
            if (bouncy) scaleIn(move(), initialScale = 0.94f, transformOrigin = TransformOrigin(0.5f, 1f)) else EnterTransition.None
    }

    fun panelExit(): ExitTransition {
        if (!enabled) return ExitTransition.None
        if (!moves) return fadeOut(tween(160))
        val fall = if (style == MotionStyle.MODERN) 0.35f else 1f
        return slideOutVertically(offsetMove) { (it * fall).roundToInt() } +
            fadeOut(tween(if (style == MotionStyle.MODERN) 200 else 120)) +
            if (bouncy) scaleOut(move(), targetScale = 0.94f, transformOrigin = TransformOrigin(0.5f, 1f)) else ExitTransition.None
    }

    /** A block that opens below what's there (a setting's options, a bar appearing). */
    fun reveal(): EnterTransition = when {
        !enabled -> EnterTransition.None
        !moves -> fadeIn(tween(160))
        else -> expandVertically(move(IntSize.VisibilityThreshold)) + fadeIn(tween(220, delay = 40))
    }

    fun conceal(): ExitTransition = when {
        !enabled -> ExitTransition.None
        !moves -> fadeOut(tween(120))
        else -> shrinkVertically(move(IntSize.VisibilityThreshold)) + fadeOut(tween(120))
    }

    /** Bounds of a shared element (the cover growing into the player). */
    fun bounds(): FiniteAnimationSpec<Rect> = move(Rect.VisibilityThreshold)

    // lazy list rows: placement when the list changes, a fade for rows that arrive (appended pages)
    fun itemFadeIn(): FiniteAnimationSpec<Float>? = if (enabled) tween(220) else null
    fun itemPlacement(): FiniteAnimationSpec<IntOffset>? = if (moves) offsetMove else null
    fun itemFadeOut(): FiniteAnimationSpec<Float>? = if (enabled) tween(120) else null

    private companion object {
        val NONE = (EnterTransition.None togetherWith ExitTransition.None).sized(null)
    }
}

val LocalMotion = staticCompositionLocalOf { MotionScheme(Motion.FULL, MotionStyle.MODERN) }

/**
 * Provides the scheme for [level] in the skin of [layout]. AUTO becomes SUBTLE while the
 * system's "Remove animations" is on (animator duration scale 0), FULL otherwise; an explicit
 * choice wins over the system. Other scales (0.5×, 2×...) already stretch or shorten every
 * Compose animation by themselves.
 */
@Composable
fun ProvideMotion(level: Motion, layout: PlayerLayout, content: @Composable () -> Unit) {
    val systemOff = rememberSystemAnimationsOff()
    val effective = level.resolve(systemOff)
    val style = layout.motionStyle()
    val scheme = remember(effective, style) { MotionScheme(effective, style) }
    CompositionLocalProvider(LocalMotion provides scheme, content = content)
}

/** The same level in another skin (the full player follows its own layout, not the app's). */
@Composable
fun ProvideMotionStyle(layout: PlayerLayout, content: @Composable () -> Unit) {
    val current = LocalMotion.current
    val style = layout.motionStyle()
    val scheme = remember(current, style) { if (current.style == style) current else MotionScheme(current.level, style) }
    CompositionLocalProvider(LocalMotion provides scheme, content = content)
}

/** True while the developer option "Animator duration scale" is off; follows changes live. */
@Composable
private fun rememberSystemAnimationsOff(): Boolean {
    val resolver = LocalContext.current.contentResolver
    var off by remember { mutableStateOf(animationsOff(resolver)) }
    DisposableEffect(resolver) {
        val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                off = animationsOff(resolver)
            }
        }
        resolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, observer)
        off = animationsOff(resolver)
        onDispose { resolver.unregisterContentObserver(observer) }
    }
    return off
}

private fun animationsOff(resolver: android.content.ContentResolver): Boolean =
    runCatching { Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f }.getOrDefault(false)

// ------------------------------------------------------------------ list entrance

/** Rows past this index never cascade (12, as on the desktop): long lists stay as fast as before. */
private const val CASCADE_ROWS = 12
private const val CASCADE_STAGGER = 35
private const val CASCADE_DURATION = 280

/**
 * One clock for the first rows of a list: row i fades and rises once the clock passes
 * i × stagger. Read only in the draw phase (graphicsLayer), so nothing recomposes while
 * it runs; once it has run, rows scrolled back into view stay still.
 */
@Stable
class ListEntrance internal constructor(internal val rise: Float) {
    internal val clock = Animatable(0f)
    internal val total = CASCADE_DURATION + CASCADE_ROWS * CASCADE_STAGGER
}

private val LocalListEntrance = compositionLocalOf<ListEntrance?> { null }

/**
 * The rows inside [content] cascade in the first time it shows, and again when [key]
 * changes (new search results). Only in FULL motion.
 */
@Composable
fun ListEntrance(key: Any? = Unit, content: @Composable () -> Unit) {
    val motion = LocalMotion.current
    val rise = with(LocalDensity.current) { 16.dp.toPx() }
    // same tree either way, so changing the setting doesn't rebuild (and rescroll) the list
    val entrance = if (motion.cascade) remember(key) { ListEntrance(rise) } else null
    LaunchedEffect(entrance) {
        entrance?.clock?.animateTo(1f, androidx.compose.animation.core.tween(motion.ms(entrance.total), easing = LinearEasing))
    }
    CompositionLocalProvider(LocalListEntrance provides entrance, content = content)
}

/**
 * Motion of a lazy row: [LazyItemScope.animateItem] (placement, and a fade for rows that are
 * inserted or appended) plus, for the first rows, the entrance cascade. Apply it to the row's
 * outermost layout.
 */
@Composable
fun LazyItemScope.itemMotion(index: Int = Int.MAX_VALUE): Modifier {
    val motion = LocalMotion.current
    if (!motion.enabled) return Modifier
    return Modifier
        .animateItem(motion.itemFadeIn(), motion.itemPlacement(), motion.itemFadeOut())
        .entrance(LocalListEntrance.current, index)
}

/** The same for a lazy grid's cells (album covers). */
@Composable
fun LazyGridItemScope.itemMotion(index: Int = Int.MAX_VALUE): Modifier {
    val motion = LocalMotion.current
    if (!motion.enabled) return Modifier
    return Modifier
        .animateItem(motion.itemFadeIn(), motion.itemPlacement(), motion.itemFadeOut())
        .entrance(LocalListEntrance.current, index)
}

private fun Modifier.entrance(entrance: ListEntrance?, index: Int): Modifier {
    if (entrance == null || index >= CASCADE_ROWS) return this
    return graphicsLayer {
        val elapsed = entrance.clock.value * entrance.total - index * CASCADE_STAGGER
        val p = (elapsed / CASCADE_DURATION).coerceIn(0f, 1f)
        // ease out: quick at first, settling at the end
        val e = 1f - (1f - p) * (1f - p)
        alpha = e
        translationY = (1f - e) * entrance.rise
    }
}

// ------------------------------------------------------------------ shared cover

/** Set by HomefiRoot in FULL motion: the scope the cover flies through. */
@OptIn(ExperimentalSharedTransitionApi::class)
val LocalSharedCover = staticCompositionLocalOf<SharedTransitionScope?> { null }

/** The visibility of the panel (mini player, full player) a shared cover belongs to. */
val LocalCoverVisibility = compositionLocalOf<AnimatedVisibilityScope?> { null }

/**
 * Marks the cover of [mediaId] as the same picture in the mini player and the full player,
 * so opening one grows it out of the other. Does nothing outside a shared scope or in
 * reduced motion.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun Modifier.sharedCover(mediaId: String?): Modifier {
    val shared = LocalSharedCover.current ?: return this
    val visibility = LocalCoverVisibility.current ?: return this
    val motion = LocalMotion.current
    if (mediaId == null || !motion.sharedCover) return this
    return with(shared) {
        this@sharedCover.sharedElement(
            rememberSharedContentState("cover-$mediaId"),
            visibility,
            boundsTransform = { _, _ -> motion.bounds() },
        )
    }
}
