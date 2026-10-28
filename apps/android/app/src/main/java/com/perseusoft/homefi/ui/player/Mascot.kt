package com.perseusoft.homefi.ui.player

import com.perseusoft.homefi.ui.haptics
import androidx.annotation.OptIn
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.util.UnstableApi
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.MascotLimbs
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.AudioTap
import kotlinx.coroutines.delay
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sign
import kotlin.math.sin

// The HomeFi mascot (a white house wearing headphones) drawn on a Canvas in
// its own units: the art is 1000 wide, shown through the desktop's viewBox
// "40 40 1000 900". Faces, costumes and fx follow the desktop's
// `components/Mascot.svelte`; the body moves with [Choreographer].

private const val VIEW_X = 40f
private const val VIEW_Y = 40f
private const val VIEW_W = 1000f
private const val VIEW_H = 900f

/** Feet on the ground: the body turns and squashes around this point. */
private val FEET = Offset(512f, 896f)
private val TILT_PIVOT = Offset(512f, 560f)
private const val TILT = -6f

// limbs (keep in sync with the desktop)
private val SHOULDER_L = Offset(268f, 730f)
private val SHOULDER_R = Offset(756f, 730f)
private const val ARM = 130f
private const val ARM_W = 56f
private const val HAND = 38f
private val HIP_L = Offset(420f, 800f)
private val HIP_R = Offset(604f, 780f)
private const val GROUND = 896f
private const val LEG_W = 62f
private const val FOOT_RX = 50f
private const val FOOT_RY = 28f
private const val FOOT_SHIFT = 20f
private const val OUTLINE = 8f

private const val LX = 440f
private const val LY = 600f
private const val RX = 584f
private const val RY = 600f
private const val EYE_R = 34f

// the guitar (keep in sync with the desktop): body across the belly on the
// picking hand's side (the left of the picture), neck slanting up towards the
// fretting hand, [GUITAR_AHEAD] in front of the house
private const val GUITAR_X = 410f
private const val GUITAR_Y = 712f
private const val GUITAR_AHEAD = 150f
private const val GUITAR_NECK = 22f
private const val GUITAR_SCALE = 1.2f
/** How far an arm stretches to reach the guitar. */
private const val ARM_MAX = 175f

// turning sideways (keep in sync with the desktop)
private const val HEAD_R = 270f
private const val BEHIND = -0.3f
private const val BAND_W = 50f

/** Art colours, same as the desktop's MASCOT_COLORS and costume palette. */
private object Ink {
    val navy = Color(0xFF0B1E3A)
    val white = Color.White
    val cushion = Color(0xFF34D399)
    val cheek = Color(0xFFF472B6)
    val note = Color(0xFFFDE68A)
    val yellow = Color(0xFFFDE047)
    val pink = Color(0xFFF472B6)
    val lightPink = Color(0xFFF9A8D4)
    val hotPink = Color(0xFFEC4899)
    val red = Color(0xFFEF4444)
    val darkRed = Color(0xFFDC2626)
    val maroon = Color(0xFF7F1D1D)
    val brick = Color(0xFF991B1B)
    val crimson = Color(0xFFB91C1C)
    val sky = Color(0xFF38BDF8)
    val amber = Color(0xFFFCD34D)
    val gold = Color(0xFFFBBF24)
    val bronze = Color(0xFFB45309)
    val brown = Color(0xFF7C2D12)
    val shades = Color(0xFF05070D)
    val leather = Color(0xFF111827)
    val steel = Color(0xFFD1D5DB)
    val silver = Color(0xFFCBD5E1)
    val green = Color(0xFF22C55E)
    val darkGreen = Color(0xFF15803D)
    val beret = Color(0xFF1E3A8A)
    val steam = Color(0xFFE5E7EB)
    val micHandle = Color(0xFF374151)
    val micRing = Color(0xFF6B7280)
    val micHead = Color(0xFFD1D5DB)
    val guitarRed = Color(0xFFDC2626)
    val pickguard = Color(0xFFF8FAFC)
    val pickup = Color(0xFF111827)
    val bridge = Color(0xFF9CA3AF)
    val fretboard = Color(0xFFA16207)
    val fretWire = Color(0xFFFDE68A)
    val strings = Color(0xFFE5E7EB)
}

/** A colour that cycles through the hues, like the desktop's `hue-rotate` neon. */
private class Neon(argb: Long) {
    private val hsv = FloatArray(3).also { android.graphics.Color.colorToHSV(argb.toInt(), it) }
    fun at(shift: Float) = Color.hsv((hsv[0] + shift) % 360f, hsv[1], hsv[2])
}

private fun svg(d: String): Path = PathParser().parsePathString(d).toPath()

private fun heart(x: Float, y: Float, s: Float) =
    "M$x ${y + s * 0.3f}C$x ${y - s * 0.2f} ${x - s} ${y - s * 0.2f} ${x - s} ${y + s * 0.35f}" +
        "C${x - s} ${y + s * 0.8f} $x ${y + s * 1.1f} $x ${y + s * 1.3f}" +
        "C$x ${y + s * 1.1f} ${x + s} ${y + s * 0.8f} ${x + s} ${y + s * 0.35f}" +
        "C${x + s} ${y - s * 0.2f} $x ${y - s * 0.2f} $x ${y + s * 0.3f}Z"

private fun star(x: Float, y: Float, r: Float, inner: Float = 0.45f) =
    "M" + (0 until 10).joinToString("L") { i ->
        val a = PI / 5 * i - PI / 2
        val d = if (i % 2 == 1) r * inner else r
        "${x + d * cos(a)} ${y + d * sin(a)}"
    } + "Z"

private fun bolt(x: Float, y: Float, s: Float) =
    "M$x ${y}L${x - s * 0.5f} ${y + s * 1.1f}L${x - s * 0.05f} ${y + s * 1.1f}L${x - s * 0.35f} ${y + s * 2}" +
        "L${x + s * 0.55f} ${y + s * 0.75f}L${x + s * 0.08f} ${y + s * 0.75f}L${x + s * 0.35f} ${y}Z"

/** Every path, parsed once (from the desktop's `lib/mascot.ts` and Mascot.svelte). */
private object Art {
    val house = svg("M542.1 288L748.1 466.2A46 46 0 0 1 764 501L764 760A46 46 0 0 1 718 806L306 806A46 46 0 0 1 260 760L260 501A46 46 0 0 1 275.9 466.2L481.9 288A46 46 0 0 1 542.1 288Z")
    // one cup / pad per side, so each can slide round the head on its own
    val cupL = svg("M236 508H248A40 40 0 0 1 288 548V664A40 40 0 0 1 248 704H236A40 40 0 0 1 196 664V548A40 40 0 0 1 236 508Z")
    val cupR = svg("M776 508H788A40 40 0 0 1 828 548V664A40 40 0 0 1 788 704H776A40 40 0 0 1 736 664V548A40 40 0 0 1 776 508Z")
    val padL = svg("M277 540H277A15 15 0 0 1 292 555V657A15 15 0 0 1 277 672H277A15 15 0 0 1 262 657V555A15 15 0 0 1 277 540Z")
    val padR = svg("M747 540H747A15 15 0 0 1 762 555V657A15 15 0 0 1 747 672H747A15 15 0 0 1 732 657V555A15 15 0 0 1 747 540Z")
    val eyes = svg("M406 600A34 34 0 1 0 474 600A34 34 0 1 0 406 600ZM550 600A34 34 0 1 0 618 600A34 34 0 1 0 550 600Z")
    val sparkles = svg("M441 586A11 11 0 1 0 463 586A11 11 0 1 0 441 586ZM585 586A11 11 0 1 0 607 586A11 11 0 1 0 585 586Z")
    val smile = svg("M432 668Q512 760 592 668Q512 716 432 668Z")
    val cheeks = svg("M354 676A28 28 0 1 0 410 676A28 28 0 1 0 354 676ZM614 676A28 28 0 1 0 670 676A28 28 0 1 0 614 676Z")
    val notes = svg("M810.5 330.7A34 25 -22 1 0 873.5 305.3A34 25 -22 1 0 810.5 330.7ZM918.5 294.7A34 25 -22 1 0 981.5 269.3A34 25 -22 1 0 918.5 294.7ZM868 156H868A8 8 0 0 1 876 164V308A8 8 0 0 1 868 316H868A8 8 0 0 1 860 308V164A8 8 0 0 1 868 156ZM976 120H976A8 8 0 0 1 984 128V272A8 8 0 0 1 976 280H976A8 8 0 0 1 968 272V128A8 8 0 0 1 976 120ZM860 150L984 114L984 152L860 188ZM103.7 309A24 18 -22 1 0 148.3 291A24 18 -22 1 0 103.7 309ZM144 196H144A6 6 0 0 1 150 202V294A6 6 0 0 1 144 300H144A6 6 0 0 1 138 294V202A6 6 0 0 1 144 196ZM138 190Q178 208 176 250Q166 226 150 222Z")

    // faces
    val closedEyes = svg("M${LX - EYE_R} ${LY}Q$LX ${LY + EYE_R * 0.9f} ${LX + EYE_R} ${LY}M${RX - EYE_R} ${RY}Q$RX ${RY + EYE_R * 0.9f} ${RX + EYE_R} $RY")
    val happyEyes = svg("M${LX - EYE_R} ${LY + 8}Q$LX ${LY - EYE_R * 0.9f} ${LX + EYE_R} ${LY + 8}M${RX - EYE_R} ${RY + 8}Q$RX ${RY - EYE_R * 0.9f} ${RX + EYE_R} ${RY + 8}")
    val coolEyes = svg("M${LX - EYE_R} ${LY}A$EYE_R $EYE_R 0 0 0 ${LX + EYE_R} ${LY}ZM${RX - EYE_R} ${RY}A$EYE_R $EYE_R 0 0 0 ${RX + EYE_R} ${RY}Z")
    val coolLids = svg("M${LX - EYE_R} ${LY}L${LX + EYE_R} ${LY}M${RX - EYE_R} ${RY}L${RX + EYE_R} $RY")
    val angryBrows = svg("M398 548L478 578M626 548L546 578")
    // a tantrum: frowning, eyes narrowed, teeth clenched, a throbbing vein
    val furiousBrows = svg("M396 536L480 574M628 536L544 574")
    private const val SQUINT = EYE_R * 0.85f
    val furiousEyes = svg(
        "M${LX - SQUINT} ${LY - 2}L${LX + SQUINT} ${LY + 10}A$SQUINT $SQUINT 0 0 1 ${LX - SQUINT} ${LY - 2}Z" +
            "M${RX + SQUINT} ${RY - 2}L${RX - SQUINT} ${RY + 10}A$SQUINT $SQUINT 0 0 0 ${RX + SQUINT} ${RY - 2}Z",
    )
    val teeth = svg("M462 704H562M485 684V724M512 684V724M539 684V724")
    val micGrille = svg("M-30 ${ARM + 92}H30M-30 ${ARM + 108}H30M-12 ${ARM + 66}V${ARM + 134}M12 ${ARM + 66}V${ARM + 134}")
    val vein = svg("M-34 -10Q-12 -12 -10 -34M10 -34Q12 -12 34 -10M34 10Q12 12 10 34M-10 34Q-12 12 -34 10")
    val smirk = svg("M470 690Q512 712 560 684")
    val calmMouth = svg("M478 688Q512 712 546 688")
    val shadesBridge = svg("M${LX + EYE_R + 22} ${LY - 10}L${RX - EYE_R - 22} ${RY - 10}")
    val shadesGlint = svg("M${LX - EYE_R - 4} ${LY - 14}L${LX - 6} ${LY - 14}M${RX - EYE_R - 4} ${RY - 14}L${RX - 6} ${RY - 14}")

    // fx
    val boltBig = svg(bolt(880f, 150f, 90f))
    val boltSmall = svg(bolt(150f, 220f, 70f))
    val shock = svg("M120 520q-40 60 0 120M80 500q-60 90 0 180M904 520q40 60 0 120M944 500q60 90 0 180")
    val heartBig = svg(heart(870f, 160f, 60f))
    val heartMid = svg(heart(160f, 250f, 42f))
    val heartSmall = svg(heart(950f, 420f, 30f))

    // makeup
    val glamBolt = svg("M392 500L470 500L438 590L494 590L388 726L416 620L366 620Z")
    val glamEdge = svg("M404 512L452 512L426 584")
    val punkBrows = svg("M392 552Q440 530 488 552M536 552Q584 530 632 552")
    val starsPink = svg(star(372f, 628f, 14f) + star(652f, 628f, 14f))
    val starsGold = svg(star(396f, 720f, 9f) + star(628f, 720f, 9f))
    val uvSmile = svg("M470 760L512 744L554 760")
    val salsaBrows = svg("M400 560Q440 536 480 560M544 560Q584 536 624 560")

    // costumes
    private val spikeXs = (0 until 9).map { 300f + it * 53f }
    val spikeXList = spikeXs.toFloatArray()
    val spikes = svg(spikeXs.joinToString("") { x -> "M${x - 14} 744L$x 712L${x + 14} 744Z" })
    val hornL = svg("M392 262Q340 200 356 128Q384 196 440 226Z")
    val hornR = svg("M632 262Q684 200 668 128Q640 196 584 226Z")
    val mohawk = svg("M432 224L436 60L478 206L496 10L524 204L552 26L568 208L600 76L594 228Z")
    val pin = svg("M300 760H372Q388 760 388 746Q388 732 372 732H312")
    val bowL = svg("M640 372L580 330Q560 372 580 414Z")
    val bowR = svg("M640 372L700 330Q720 372 700 414Z")
    val lashes = svg("M402 582L388 568M414 572L406 554M626 572L634 554M638 582L652 568")
    val glowNecklace = svg("M290 770Q512 830 734 770")
    val capCrown = svg("M372 230Q380 120 512 112Q644 120 652 230Z")
    val capBrim = svg("M372 230Q300 236 268 212Q300 196 380 204Z")
    val chain = svg("M320 744Q512 820 704 744")
    val medalNote = svg("M504 774V800M504 774L524 768V794")
    val glassesBridge = svg("M${LX + 50} ${LY}Q512 ${LY - 16} ${RX - 50} $RY")
    val bowTie = svg("M512 784L456 756V812ZM512 784L568 756V812Z")

    // the guitar, neck along +x, centred on its body
    val headOutline = svg("M352 -36L446 -46Q458 0 446 46L352 36Z")
    val head = svg("M358 -30L440 -38Q450 0 440 38L358 30Z")
    val pickguard = svg("M-70 -40Q-10 -66 60 -30L60 30Q-10 66 -70 40Z")
    val frets = svg(listOf(110, 150, 186, 220, 252, 282, 310, 336).joinToString("") { "M$it -17V17" })
    val strings = svg("M-64 -12H372M-64 0H372M-64 12H372")
    val pick = svg("M-22 0L22 0L0 40Z")

    /** A "z" for the sleeping mascot, in a unit square. */
    val zed = svg("M0 0H1L0 1H1")

    val uvTeal = Neon(0xFF5EEAD4)
    val uvPink = Neon(0xFFE879F9)
}

/** Strokes in mascot units, created once. */
private object Pen {
    fun round(w: Float) = Stroke(w, cap = StrokeCap.Round, join = StrokeJoin.Round)
    val w5 = round(5f)
    val w6 = round(6f)
    val w7 = round(7f)
    val w8 = round(8f)
    val w9 = Stroke(9f)
    val w10 = round(10f)
    val w12 = round(12f)
    val w14 = round(14f)
    val w16 = round(16f)
    val w18 = round(18f)
    val w20 = round(20f)
    val plain8 = Stroke(8f, join = StrokeJoin.Round)
    val zed = round(0.2f)
    val chain = Stroke(10f, cap = StrokeCap.Round, pathEffect = PathEffect.dashPathEffect(floatArrayOf(14f, 6f)))
}

/** What changes from frame to frame (plain fields: the frame counter drives redraws). */
private class Frame {
    var pose = REST
    var seconds = 0.0
    /** Seconds into a tantrum, or -1 when calm. */
    var anger = -1.0
}

/** Counts quick taps; one too many and it throws a tantrum for a few seconds. */
private class Tantrum {
    private val taps = ArrayDeque<Double>()
    private var from = 0.0
    private var until = 0.0

    /** Records a tap at [now]; true when it made the mascot angry (or angrier). */
    fun tap(now: Double, temper: Temper): Boolean {
        if (now < until) {
            // poking an angry mascot keeps it angry for longer
            until = min(until + 0.6, now + 2 * ANGER_TIME)
            return true
        }
        if (temper.taps == 0) return false
        while (taps.isNotEmpty() && now - taps.first() >= TEMPER_WINDOW) taps.removeFirst()
        taps.addLast(now)
        if (taps.size < temper.taps) return false
        taps.clear()
        from = now
        until = now + ANGER_TIME
        return true
    }

    fun at(now: Double) = if (now < until) now - from else -1.0
}

/**
 * The HomeFi mascot dancing to the music: its groove and costume follow the
 * genre (or the user's pick), famous dances come up now and then. Tap it to
 * make it dance (or wave while paused; too many taps and it gets angry),
 * long-press for the next style, drag to move it, pinch to resize, double-tap
 * to put it back.
 */
@OptIn(UnstableApi::class)
@Composable
fun MascotView(modifier: Modifier = Modifier, singing: Boolean = false) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    // the player around it: this phone's, or the hub's song on a guest
    val player by playerSource().state.collectAsStateWithLifecycle()
    DisposableEffect(Unit) {
        AudioTap.watchers++
        onDispose { AudioTap.watchers-- }
    }

    val current = player.current
    LaunchedEffect(current?.mediaId) { AudioTap.listenTo(current?.mediaId) }
    // the sound only gets a say after a few seconds, so look again now and then
    val heardMood by produceState(Mood.BOP, current?.mediaId, current?.genre, current?.title, current?.album) {
        val description = listOfNotNull(current?.title, current?.album).joinToString(" ")
        while (true) {
            value = moodFor(current?.genre, description, AudioTap.listening.value)
            delay(1000)
        }
    }
    val mood = settings.mascotMood ?: heardMood

    val playing by rememberUpdatedState(player.isPlaying)
    val moodNow by rememberUpdatedState(mood)
    val specials by rememberUpdatedState(settings.mascotSpecials)
    val pinned by rememberUpdatedState(settings.mascotDance)
    val turns by rememberUpdatedState(settings.mascotTurns)
    val temper by rememberUpdatedState(settings.mascotTemper)
    val longPress by rememberUpdatedState(settings.mascotLongPress)
    val sing by rememberUpdatedState(singing)
    val choreo = remember { Choreographer() }
    val frame = remember { Frame() }
    val tantrum = remember { Tantrum() }
    val haptics = haptics()
    // the style a long-press just switched to, shown for a moment
    var styleTag by remember { mutableStateOf<Int?>(null) }
    var styleTagCount by remember { mutableIntStateOf(0) }
    LaunchedEffect(styleTagCount) {
        delay(1400)
        styleTag = null
    }
    val tick = remember { mutableLongStateOf(0L) }
    LaunchedEffect(choreo) {
        var last = 0L
        while (true) {
            withFrameNanos { now ->
                val dt = if (last == 0L) 0.0 else ((now - last) / 1e9).coerceIn(0.0, 0.1)
                last = now
                frame.pose = choreo.step(dt, playing, AudioTap.listening.value.bpm.toDouble(), AudioTap.energy.toDouble(), moodNow, specials, pinned, turns)
                frame.seconds = now / 1e9
                frame.anger = tantrum.at(frame.seconds)
                if (frame.anger >= 0) frame.pose = angryPose(frame.anger)
                // karaoke: the right hand holds the microphone up to the mouth
                else if (sing && playing) frame.pose = frame.pose.copy(armR = -110.0, turn = 0.0, flip = false, guitar = 0.0)
                tick.longValue = now
            }
        }
    }

    // where the user put it; saved once they stop moving it
    var dx by remember { mutableFloatStateOf(settings.mascotX) }
    var dy by remember { mutableFloatStateOf(settings.mascotY) }
    var zoom by remember { mutableFloatStateOf(settings.mascotScale) }
    // a reset from Settings
    LaunchedEffect(settings.mascotX, settings.mascotY, settings.mascotScale) {
        dx = settings.mascotX
        dy = settings.mascotY
        zoom = settings.mascotScale
    }
    LaunchedEffect(dx, dy, zoom) {
        delay(400)
        val s = app.settings.current
        if (s.mascotX != dx || s.mascotY != dy || s.mascotScale != zoom) {
            app.settings.update { it.copy(mascotX = dx, mascotY = dy, mascotScale = zoom) }
        }
    }

    val limbs = settings.mascotLimbs
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val ground = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f)
    val description = stringResource(R.string.mascot_description)

    BoxWithConstraints(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        val density = LocalDensity.current
        val stageW = with(density) { maxWidth.toPx() }
        val stageH = with(density) { maxHeight.toPx() }
        val width = min(maxWidth.value, maxHeight.value * VIEW_W / VIEW_H) * 0.92f * zoom
        Box(
            Modifier
                .offset { IntOffset((dx * stageW).roundToInt(), (dy * stageH).roundToInt()) }
                .requiredSize(width.dp, (width * VIEW_H / VIEW_W).dp)
                .semantics { contentDescription = description }
                .pointerInput(Unit) {
                    detectTapGestures(
                        onTap = {
                            if (!tantrum.tap(frame.seconds, temper)) {
                                if (playing) choreo.trigger(moodNow) else choreo.greet()
                            }
                        },
                        onLongPress = {
                            if (longPress) {
                                // Auto, then each style in turn, then Auto again
                                val order = listOf(null) + Mood.entries
                                val next = order[(order.indexOf(app.settings.current.mascotMood) + 1) % order.size]
                                app.settings.update { it.copy(mascotMood = next) }
                                haptics.longPress()
                                styleTag = next?.label ?: R.string.mascot_auto
                                styleTagCount++
                            }
                        },
                        onDoubleTap = {
                            dx = 0f
                            dy = 0f
                            zoom = 1f
                        },
                    )
                }
                .pointerInput(stageW, stageH) {
                    detectTransformGestures { _, pan, scale, _ ->
                        zoom = (zoom * scale).coerceIn(0.4f, 1.8f)
                        dx = (dx + pan.x / stageW).coerceIn(-0.5f, 0.5f)
                        dy = (dy + pan.y / stageH).coerceIn(-0.5f, 0.5f)
                    }
                },
        ) {
            Canvas(Modifier.fillMaxSize()) {
                tick.longValue // redraw every frame
                val s = size.width / VIEW_W
                withTransform({
                    scale(s, s, Offset.Zero)
                    translate(-VIEW_X, -VIEW_Y)
                }) {
                    drawMascot(frame.pose, mood, player.isPlaying, limbs, accent, muted, ground, frame.seconds, choreo.beat, frame.anger,
                        if (singing && player.isPlaying && frame.anger < 0) AudioTap.energy else -1f)
                }
            }
            styleTag?.let { tag ->
                Text(
                    stringResource(tag),
                    Modifier
                        .align(Alignment.TopCenter)
                        .background(accent, RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 3.dp),
                    color = MaterialTheme.colorScheme.onPrimary,
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
    }
}

/**
 * The whole mascot in its own units. [t] is a clock in seconds, [beat] the
 * dance's beat count, [anger] seconds into a tantrum (-1 when calm), [sing]
 * the loudness while it sings karaoke (-1 when it doesn't).
 */
private fun DrawScope.drawMascot(
    p: Pose,
    mood: Mood,
    playing: Boolean,
    limbs: MascotLimbs,
    accent: Color,
    muted: Color,
    ground: Color,
    t: Double,
    beat: Double,
    anger: Double,
    sing: Float,
) {
    val hue = ((t % 1.6) / 1.6 * 360).toFloat()
    val angry = anger >= 0

    // a shadow on the floor that shrinks as it jumps
    val air = (-p.y / 200).coerceIn(0.0, 0.5).toFloat()
    val rx = 230f * (1 - air)
    drawOval(ground, Offset(512f + p.x.toFloat() - rx, GROUND + 8f), Size(2 * rx, 28f))

    when {
        angry -> Unit
        playing -> drawFx(mood, t, hue)
        else -> drawZeds(t, muted)
    }

    withTransform({
        translate(p.x.toFloat(), p.y.toFloat())
        rotate(p.rot.toFloat(), FEET)
        scale(p.sx.toFloat() * if (p.flip) -1f else 1f, p.sy.toFloat(), FEET)
    }) {
        val fill = when (limbs) {
            MascotLimbs.WHITE -> Ink.white
            MascotLimbs.NAVY -> Ink.navy
            MascotLimbs.ACCENT -> accent
            MascotLimbs.OFF -> null
        }
        val outline = if (limbs == MascotLimbs.WHITE) Ink.navy else null
        val turn = Turn(p.turn.toFloat())
        // legs: outside the tilt, behind the house; toes point where it faces
        if (fill != null) {
            listOf(turn.limb(HIP_L, p.legL.toFloat(), 1f) to p.liftL, turn.limb(HIP_R, p.legR.toFloat(), -1f) to p.liftR)
                .sortedBy { it.first.depth }
                .forEach { (leg, lift) -> drawLeg(leg, lift.toFloat(), fill, outline) }
        }
        rotate(TILT, TILT_PIVOT) {
            val g = if (fill == null) 0f else p.guitar.toFloat().coerceIn(0f, 1f)
            val guitar = Guitar(turn)
            val arms = if (fill == null) emptyList() else {
                listOf(
                    Arm.of(turn.limb(SHOULDER_L, p.armL.toFloat(), 1f), Hold.PICK),
                    Arm.of(turn.limb(SHOULDER_R, p.armR.toFloat(), -1f), Hold.FRET),
                ).map { if (g > 0) it.reaching(if (it.hold == Hold.PICK) guitar.pickAt(p.strum.toFloat(), turn.side) else guitar.fretAt(p.fret.toFloat(), it.at), g) else it }
                    .sortedBy { it.depth }
            }
            // with the guitar on, the fretting arm stays in front of the house (it reaches round to the neck)
            val behind = arms.filter { it.depth < BEHIND && !(g > 0.5f && it.hold == Hold.FRET) }
            val pick = if (g > 0.5f) -GUITAR_NECK + 14 * p.strum.toFloat() else null
            // the microphone is in the right hand (the one whose shoulder is on the right)
            val micArm = if (sing >= 0) arms.maxByOrNull { it.at.x } else null
            val cups = turn.cups()
            val band = turn.bandHalves(cups)
            val bandPen = Pen.round(BAND_W * (1 - 0.45f * abs(turn.side)))
            behind.forEach { drawArm(it, fill!!, outline, mic = it === micArm) }
            // cups and the band half reaching them go behind as soon as they face away
            band.filter { it.second < 0 }.forEach { drawBand(it.first, bandPen, mood, playing, hue, beat) }
            cups.filter { it.depth < 0 }.forEach { drawCup(it, mood) }

            withTransform({ scale(turn.narrow, 1f, Offset(512f, 0f)) }) {
                drawPath(Art.house, Ink.white)
                // red in the face, pulsing; fades in at the start
                if (angry) drawPath(Art.house, Ink.red, alpha = (min(1.0, anger / 0.3) * (0.22 + 0.1 * sin(2 * PI * 3 * anger))).toFloat())
                drawBodyDecor(mood, hue)
                clipPath(Art.house) {
                    withTransform({
                        translate(turn.faceShift, 0f)
                        scale(turn.faceScale, 1f, Offset(512f, 0f))
                    }) {
                        drawMakeup(mood, hue)
                        when {
                            angry -> drawFuriousFace()
                            sing >= 0 -> drawSingingFace(sing)
                            else -> drawFace(mood, playing, t)
                        }
                        drawPath(Art.cheeks, Ink.cheek, alpha = if (playing && mood == Mood.CUTE) 0.9f else 0.6f)
                        drawFaceDecor(mood)
                    }
                }
            }

            band.filter { it.second >= 0 }.forEach { drawBand(it.first, bandPen, mood, playing, hue, beat) }
            cups.filter { it.depth >= 0 }.forEach { drawCup(it, mood) }
            withTransform({
                scale(turn.narrow, 1f, Offset(512f, 0f))
                translate(turn.topShift, 0f)
                scale(turn.topScale, 1f, Offset(512f, 0f))
            }) {
                drawTopDecor(mood)
                if (angry) {
                    val throb = (1 + 0.18 * abs(sin(2 * PI * 2.5 * anger))).toFloat()
                    withTransform({
                        translate(652f, 420f)
                        scale(throb, throb, Offset.Zero)
                    }) { drawPath(Art.vein, Ink.darkRed, style = Pen.w12) }
                }
            }
            // in front of the body, behind the hands
            if (g > 0) drawGuitar(guitar, min(1f, g * 2))
            // arms in front, so raised arms don't hide behind the headphones
            arms.filter { it !in behind }.forEach { drawArm(it, fill!!, outline, mic = it === micArm, pick = if (it.hold == Hold.PICK) pick else null) }
        }
    }
    if (angry) drawSteam(anger)
}

/** Puffs of steam out of each headphone cup. */
private fun DrawScope.drawSteam(anger: Double) {
    for (i in 0 until 3) {
        val f = ((anger * 1.4 + i / 3.0) % 1.0).toFloat()
        val alpha = (min(1.0, anger / 0.4) * 0.85).toFloat() * (1 - f)
        val r = 22f + 40f * f
        val y = 520f - 230f * f
        for (x in floatArrayOf(200f - 70f * f, 824f + 70f * f)) {
            drawCircle(Ink.steam, r, Offset(x, y), alpha = alpha)
            // a faint edge so it shows on light backgrounds too
            drawCircle(Ink.navy, r, Offset(x, y), alpha = alpha * 0.18f, style = Pen.w5)
        }
    }
}

private fun DrawScope.drawFuriousFace() {
    drawPath(Art.furiousBrows, Ink.navy, style = Pen.w20)
    drawPath(Art.furiousEyes, Ink.navy)
    drawRoundRect(Ink.white, Offset(458f, 682f), Size(108f, 44f), CornerRadius(12f))
    drawRoundRect(Ink.navy, Offset(458f, 682f), Size(108f, 44f), CornerRadius(12f), style = Pen.w9)
    drawPath(Art.teeth, Ink.navy, style = Pen.w6)
}

/**
 * Turning sideways, drawn as if the head were a cylinder: the face and the
 * headphone cups slide round it, the house gets narrower (its side is not as
 * wide as its front) and the limbs on the far side go behind the body.
 */
private class Turn(turn: Float) {
    val th = turn * PI / 2
    val side = sin(th).toFloat()
    val across = cos(th).toFloat()
    val narrow = 1 - 0.2f * abs(side)
    // headphones sit a little behind the middle of the head when seen from the side
    private val back = -46f * side
    // the face turns less than the body so a profile still shows one eye
    private val faceTh = turn * 55 * PI / 180
    val faceShift = 210 * sin(faceTh).toFloat()
    val faceScale = cos(faceTh).toFloat()
    val topShift = 110 * sin(faceTh).toFloat()
    val topScale = 1 - 0.6f * (1 - cos(faceTh).toFloat())

    class Cup(val home: Float, val x: Float, val depth: Float, val widen: Float, val cup: Path, val pad: Path)

    fun cups() = listOf(
        Cup(242f, 512 + back - HEAD_R * narrow * across, side, widen(side), Art.cupL, Art.padL),
        Cup(782f, 512 + back + HEAD_R * narrow * across, -side, widen(-side), Art.cupR, Art.padR),
    )

    private fun widen(depth: Float) = 1 + 0.45f * max(0f, depth) * abs(side)

    /**
     * The head band, split at its top: each half goes with its cup (and its
     * depth), so the half reaching the far cup passes behind the head instead
     * of across the face.
     */
    fun bandHalves(cups: List<Cup>): List<Pair<Path, Float>> {
        val l = cups[0].x - 16
        val r = cups[1].x + 16
        val m = (l + r) / 2
        val left = Path().apply {
            moveTo(l, 560f)
            cubicTo(l, 344f, (3 * l + r) / 4, 236f, m, 236f)
        }
        val right = Path().apply {
            moveTo(m, 236f)
            cubicTo((l + 3 * r) / 4, 236f, r, 344f, r, 560f)
        }
        return listOf(left to cups[0].depth, right to cups[1].depth)
    }

    /** Where a limb joins the body and how it is rotated (degrees, clockwise). */
    class Limb(val at: Offset, val rot: Float, val foot: Float, val depth: Float)

    /** [front] is the rotation direction when facing you (+1 left, -1 right); in profile both swing towards where it faces. */
    fun limb(joint: Offset, angle: Float, front: Float): Limb {
        val towards = if (side == 0f) front else -sign(side)
        val s = abs(side)
        return Limb(
            at = Offset(512 + (joint.x - 512) * narrow * across + 18 * side, joint.y),
            rot = angle * (front + (towards - front) * s),
            foot = if (front > 0) -1 + (sign(side) + 1) * s else 1 + (sign(side) - 1) * s,
            depth = front * side,
        )
    }
}

/** Half of the head band, with the LEDs chasing round it in party. */
private fun DrawScope.drawBand(half: Path, pen: Stroke, mood: Mood, playing: Boolean, hue: Float, beat: Double) {
    drawPath(half, Ink.navy, style = pen)
    if (mood == Mood.PARTY) {
        val chase = if (playing) ((beat / 2) % 1.0 * 80).toFloat() else 0f
        drawPath(half, Art.uvTeal.at(hue), style = Stroke(10f, cap = StrokeCap.Round, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 34f), chase)))
    }
}

/** One headphone cup (and the hoop hanging from that ear in salsa). */
private fun DrawScope.drawCup(c: Turn.Cup, mood: Mood) {
    withTransform({
        translate(c.x, 0f)
        scale(c.widen, 1f, Offset.Zero)
        translate(-c.home, 0f)
    }) {
        drawPath(c.cup, Ink.navy)
        drawPath(c.pad, Ink.cushion)
        if (mood == Mood.SALSA) drawCircle(Ink.gold, 26f, Offset(c.home, 738f), style = Pen.w9)
    }
}

private enum class Hold { PICK, FRET }

/** An arm to draw: where it joins, its angle (clockwise from hanging down) and length, and what it holds on the guitar. */
private class Arm(val at: Offset, val rot: Float, val len: Float, val depth: Float, val hold: Hold) {
    /** Points it at [target], stretching up to [ARM_MAX]; [g] blends from its dance angle. */
    fun reaching(target: Offset, g: Float): Arm {
        val dx = target.x - at.x
        val dy = target.y - at.y
        val aim = Math.toDegrees(atan2(-dx, dy).toDouble()).toFloat()
        val by = ((aim - rot) % 360 + 540) % 360 - 180
        return Arm(at, rot + by * g, ARM + (min(ARM_MAX, hypot(dx, dy)) - ARM) * g, depth, hold)
    }

    companion object {
        fun of(limb: Turn.Limb, hold: Hold) = Arm(limb.at, limb.rot, ARM, limb.depth, hold)
    }
}

/**
 * Where the guitar is when turned by [turn]: it sits in front of the house, so
 * it slides round towards where the mascot faces and is seen a little edge-on
 * (it turns less than the body, so its face stays in view).
 */
private class Guitar(turn: Turn) {
    val x = 512 + (GUITAR_X - 512) * turn.across + GUITAR_AHEAD * turn.side
    val y = GUITAR_Y - 16 * abs(turn.side)
    val sx = cos(turn.th * 0.7).toFloat()
    private val cosN = cos(GUITAR_NECK * PI / 180).toFloat()
    private val sinN = sin(GUITAR_NECK * PI / 180).toFloat()

    /** A point on the guitar, [u] along the neck and [v] across it, from the middle of its body. */
    fun at(u: Float, v: Float) = Offset(x + (u * cosN + v * sinN) * sx * GUITAR_SCALE, y + (v * cosN - u * sinN) * GUITAR_SCALE)

    /** Picking hand: a short stroke across the strings over the pickups; turned, it flicks more along the face. */
    fun pickAt(strum: Float, side: Float) =
        at(-24 - 8 * strum * strum + 16 * strum * abs(side), -6 + 16 * strum * (1 - 0.4f * abs(side)))

    /** Fretting hand: on the neck, as far up as the arm reaches from [shoulder]. */
    fun fretAt(fret: Float, shoulder: Offset): Offset {
        var u = 230 + 70 * fret
        while (u > 120 && (at(u, 0f) - shoulder).getDistance() > ARM_MAX - 15) u -= 10
        return at(u, 0f)
    }
}

/** An electric guitar: red body, pickups, a neck with frets and a headstock with its pegs. */
private fun DrawScope.drawGuitar(guitar: Guitar, alpha: Float) {
    withTransform({
        translate(guitar.x, guitar.y)
        scale(guitar.sx * GUITAR_SCALE, GUITAR_SCALE, Offset.Zero)
        rotate(-GUITAR_NECK, Offset.Zero)
    }) {
        // outline first, then the parts on top
        drawCircle(Ink.navy, 92f, Offset(-38f, 0f), alpha = alpha)
        drawCircle(Ink.navy, 72f, Offset(46f, 0f), alpha = alpha)
        drawRoundRect(Ink.navy, Offset(70f, -25f), Size(300f, 50f), CornerRadius(12f), alpha = alpha)
        drawPath(Art.headOutline, Ink.navy, alpha = alpha)
        drawPath(Art.headOutline, Ink.navy, alpha = alpha, style = Pen.w12)
        drawCircle(Ink.guitarRed, 82f, Offset(-38f, 0f), alpha = alpha)
        drawCircle(Ink.guitarRed, 62f, Offset(46f, 0f), alpha = alpha)
        drawPath(Art.pickguard, Ink.pickguard, alpha = alpha * 0.9f)
        drawRoundRect(Ink.pickup, Offset(-24f, -34f), Size(18f, 68f), CornerRadius(5f), alpha = alpha)
        drawRoundRect(Ink.pickup, Offset(18f, -34f), Size(18f, 68f), CornerRadius(5f), alpha = alpha)
        drawRoundRect(Ink.bridge, Offset(-72f, -30f), Size(14f, 60f), CornerRadius(4f), alpha = alpha)
        drawCircle(Ink.yellow, 9f, Offset(-88f, 44f), alpha = alpha)
        drawCircle(Ink.yellow, 9f, Offset(-62f, 62f), alpha = alpha)
        drawRoundRect(Ink.fretboard, Offset(76f, -17f), Size(290f, 34f), CornerRadius(6f), alpha = alpha)
        drawPath(Art.frets, Ink.fretWire, alpha = alpha, style = Stroke(4f))
        drawPath(Art.head, Ink.pickup, alpha = alpha)
        for (x in listOf(376f, 402f, 428f)) {
            drawCircle(Ink.steel, 9f, Offset(x, -44f), alpha = alpha)
            drawCircle(Ink.steel, 9f, Offset(x, 44f), alpha = alpha)
        }
        drawPath(Art.strings, Ink.strings, alpha = alpha, style = Stroke(3f))
    }
}

/** One arm from its shoulder, rotated clockwise by [Arm.rot] from hanging down; [pick] is the angle of a guitar pick in its hand. */
private fun DrawScope.drawArm(arm: Arm, fill: Color, outline: Color?, mic: Boolean = false, pick: Float? = null) {
    val a = Math.toRadians(arm.rot.toDouble())
    val shoulder = arm.at
    val hand = Offset(shoulder.x - arm.len * sin(a).toFloat(), shoulder.y + arm.len * cos(a).toFloat())
    if (pick != null) {
        // held tip down towards the strings, flicking with the wrist
        withTransform({
            translate(hand.x, hand.y)
            rotate(pick, Offset.Zero)
            translate(0f, 34f)
        }) {
            drawPath(Art.pick, Ink.yellow)
            drawPath(Art.pick, Ink.navy, style = Pen.w6)
        }
    }
    if (outline != null) {
        drawLine(outline, shoulder, hand, ARM_W + 2 * OUTLINE, StrokeCap.Round)
        drawCircle(outline, HAND + OUTLINE, hand)
    }
    drawLine(fill, shoulder, hand, ARM_W, StrokeCap.Round)
    drawCircle(fill, HAND, hand)
    if (mic) {
        // drawn along the arm, pointing at the mouth
        withTransform({
            translate(shoulder.x, shoulder.y)
            rotate(arm.rot, Offset.Zero)
        }) {
            drawRoundRect(Ink.micHandle, Offset(-15f, ARM - 6), Size(30f, 78f), CornerRadius(12f))
            drawRoundRect(Ink.micRing, Offset(-19f, ARM + 58), Size(38f, 14f), CornerRadius(5f))
            drawCircle(Ink.micHead, 36f, Offset(0f, ARM + 100))
            drawCircle(Ink.micRing, 36f, Offset(0f, ARM + 100), style = Pen.w6)
            drawPath(Art.micGrille, Ink.micRing, alpha = 0.7f, style = Stroke(4f))
        }
    }
}

/** Singing its heart out: eyes shut, mouth opening with the music ([loud] 0..1). */
private fun DrawScope.drawSingingFace(loud: Float) {
    val open = min(1f, loud * 1.8f)
    drawPath(Art.happyEyes, Ink.navy, style = Pen.w12)
    val rx = 26f + 8f * min(1f, loud * 1.6f)
    val ry = 10f + 30f * open
    drawOval(Ink.navy, Offset(512f - rx, 704f - ry), Size(2 * rx, 2 * ry))
    val ty = 4f + 8f * open
    drawOval(Ink.pink, Offset(512f - 16f, 712f + 18f * open - ty), Size(32f, 2 * ty))
}

/** One leg straight down to the ground, lifted and turned (degrees, clockwise) around the hip. */
private fun DrawScope.drawLeg(leg: Turn.Limb, lift: Float, fill: Color, outline: Color?) {
    val hip = leg.at
    withTransform({
        translate(0f, -lift)
        rotate(leg.rot, hip)
    }) {
        val end = Offset(hip.x, GROUND)
        val foot = Offset(hip.x + leg.foot * FOOT_SHIFT, GROUND)
        if (outline != null) {
            drawLine(outline, hip, end, LEG_W + 2 * OUTLINE, StrokeCap.Round)
            val rx = FOOT_RX + OUTLINE
            val ry = FOOT_RY + OUTLINE
            drawOval(outline, Offset(foot.x - rx, foot.y - ry), Size(2 * rx, 2 * ry))
        }
        drawLine(fill, hip, end, LEG_W, StrokeCap.Round)
        drawOval(fill, Offset(foot.x - FOOT_RX, foot.y - FOOT_RY), Size(2 * FOOT_RX, 2 * FOOT_RY))
    }
}

/** Things floating around, depending on the mood. */
private fun DrawScope.drawFx(mood: Mood, t: Double, hue: Float) {
    val (fx, fy) = if (mood == Mood.MOSH) {
        // a jittery shake
        if (((t / 0.105).toLong() % 2L) == 0L) -6f to 4f else 6f to -4f
    } else {
        val f = (0.5 - 0.5 * cos(2 * PI * t / 2.2)).toFloat()
        10f * f to -26f * f
    }
    translate(fx, fy) {
        when (mood) {
            Mood.MOSH -> {
                drawPath(Art.boltBig, Ink.yellow)
                drawPath(Art.boltSmall, Ink.yellow)
                drawPath(Art.shock, Ink.yellow, alpha = 0.8f, style = Pen.w16)
            }
            Mood.CUTE -> {
                drawPath(Art.heartBig, Ink.pink)
                drawPath(Art.heartMid, Ink.lightPink)
                drawPath(Art.heartSmall, Ink.pink)
            }
            Mood.PARTY -> drawPath(Art.notes, Art.uvTeal.at(hue))
            else -> drawPath(Art.notes, Ink.note)
        }
    }
}

private val ZED_SIZES = floatArrayOf(40f, 52f, 80f)

/** Three z's drifting up while it sleeps. */
private fun DrawScope.drawZeds(t: Double, color: Color) {
    for (i in 0 until 3) {
        val phase = (((t - i) % 3.0 + 3.0) % 3.0 / 3.0).toFloat()
        val alpha = if (phase < 0.3f) phase / 0.3f * 0.9f else (1 - phase) / 0.7f * 0.9f
        val e = phase * phase // ease-in
        val h = ZED_SIZES[i]
        withTransform({
            translate(790f + 110f * e, 300f - 200f * e - h)
            scale(h, h, Offset.Zero)
        }) {
            drawPath(Art.zed, color, alpha = alpha, style = Pen.zed)
        }
    }
}

/** Makeup, under the eyes. */
private fun DrawScope.drawMakeup(mood: Mood, hue: Float) {
    when (mood) {
        Mood.MOSH -> {
            // Aladdin Sane bolt across the left eye
            drawPath(Art.glamBolt, Ink.red, alpha = 0.92f)
            drawPath(Art.glamEdge, Ink.sky, style = Pen.plain8)
        }
        Mood.POGO -> {
            drawPath(Art.punkBrows, Ink.navy, style = Pen.w12)
            // X plaster
            rotate(30f, Offset(660f, 700f)) {
                drawRoundRect(Ink.amber, Offset(620f, 688f), Size(80f, 24f), CornerRadius(8f))
                drawRoundRect(Ink.amber, Offset(648f, 660f), Size(24f, 80f), CornerRadius(8f))
            }
        }
        Mood.CUTE -> {
            drawPath(Art.starsPink, Ink.lightPink)
            drawPath(Art.starsGold, Ink.yellow)
        }
        Mood.PARTY -> {
            // UV face paint
            val teal = Art.uvTeal.at(hue)
            val pink = Art.uvPink.at(hue)
            drawCircle(teal, 9f, Offset(380f, 660f))
            drawCircle(pink, 9f, Offset(400f, 690f))
            drawCircle(teal, 9f, Offset(360f, 690f))
            drawCircle(pink, 9f, Offset(644f, 660f))
            drawCircle(teal, 9f, Offset(624f, 690f))
            drawCircle(pink, 9f, Offset(664f, 690f))
            drawPath(Art.uvSmile, teal, style = Pen.w8)
        }
        Mood.SALSA -> drawPath(Art.salsaBrows, Ink.brown, style = Pen.w10)
        else -> Unit
    }
}

private fun DrawScope.drawFace(mood: Mood, playing: Boolean, t: Double) {
    when {
        !playing -> {
            drawPath(Art.closedEyes, Ink.navy, style = Pen.w12)
            drawOval(Ink.navy, Offset(512f - 18f, 690f - 12f), Size(36f, 24f))
        }
        mood == Mood.MOSH -> {
            drawPath(Art.angryBrows, Ink.navy, style = Pen.w18)
            drawCircle(Ink.navy, EYE_R * 0.7f, Offset(LX, LY + 8))
            drawCircle(Ink.navy, EYE_R * 0.7f, Offset(RX, RY + 8))
            drawOval(Ink.navy, Offset(512f - 46f, 704f - 40f), Size(92f, 80f))
            drawOval(Ink.pink, Offset(512f - 26f, 724f - 14f), Size(52f, 28f))
        }
        mood == Mood.PARTY -> {
            // sunglasses
            drawRoundRect(Ink.shades, Offset(LX - EYE_R - 22, LY - 30), Size(EYE_R * 2 + 44, 62f), CornerRadius(22f))
            drawRoundRect(Ink.shades, Offset(RX - EYE_R - 22, RY - 30), Size(EYE_R * 2 + 44, 62f), CornerRadius(22f))
            drawPath(Art.shadesBridge, Ink.shades, style = Stroke(14f))
            drawPath(Art.shadesGlint, Art.uvTeal.at(0f), alpha = 0.8f, style = Pen.w8)
            drawPath(Art.smile, Ink.navy)
        }
        mood == Mood.GROOVE -> {
            drawPath(Art.coolEyes, Ink.navy)
            drawPath(Art.coolLids, Ink.navy, style = Pen.w10)
            drawPath(Art.smirk, Ink.navy, style = Pen.w14)
        }
        mood == Mood.CHILL -> {
            drawPath(Art.happyEyes, Ink.navy, style = Pen.w12)
            drawPath(Art.calmMouth, Ink.navy, style = Pen.w12)
        }
        else -> {
            // cute, pogo, salsa, bop: big shiny eyes that blink now and then
            val c = (t % 4.2 / 4.2).toFloat()
            val blink = when {
                c < 0.94f -> 1f
                c < 0.97f -> 1f - (c - 0.94f) / 0.03f * 0.9f
                else -> 0.1f + (c - 0.97f) / 0.03f * 0.9f
            }
            withTransform({ scale(1f, blink, Offset(512f, 600f)) }) {
                drawPath(Art.eyes, Ink.navy)
                drawPath(Art.sparkles, Ink.white)
                if (mood == Mood.CUTE) {
                    drawCircle(Ink.white, 6f, Offset(LX - 12, LY + 14))
                    drawCircle(Ink.white, 6f, Offset(RX - 12, RY + 14))
                }
            }
            if (mood == Mood.POGO) drawOval(Ink.navy, Offset(512f - 30f, 700f - 28f), Size(60f, 56f))
            else drawPath(Art.smile, Ink.navy)
        }
    }
}

// Costume: tells the genres apart at a glance (kept while sleeping). Split by
// where it is worn, so it turns the right way.

/** Worn round the body: stays put when it turns. */
private fun DrawScope.drawBodyDecor(mood: Mood, hue: Float) {
    when (mood) {
        Mood.MOSH -> {
            // studded collar
            drawRoundRect(Ink.leather, Offset(262f, 744f), Size(500f, 40f), CornerRadius(8f))
            drawPath(Art.spikes, Ink.steel)
            for (x in Art.spikeXList) drawCircle(Ink.steel, 7f, Offset(x, 764f))
        }
        Mood.POGO -> {
            // safety pin
            rotate(-20f, Offset(330f, 760f)) {
                drawPath(Art.pin, Ink.silver, style = Pen.w7)
                drawCircle(Ink.silver, 8f, Offset(298f, 760f), style = Pen.w7)
            }
        }
        // glow-stick necklace
        Mood.PARTY -> drawPath(Art.glowNecklace, Art.uvPink.at(hue), style = Pen.w14)
        Mood.GROOVE -> {
            // gold chain + medallion
            drawPath(Art.chain, Ink.gold, style = Pen.chain)
            drawCircle(Ink.gold, 28f, Offset(512f, 790f))
            drawCircle(Ink.bronze, 28f, Offset(512f, 790f), style = Pen.w5)
            drawPath(Art.medalNote, Ink.bronze, style = Pen.w5)
        }
        else -> Unit
    }
}

/** On top of the head: turns with the face, a little less. */
private fun DrawScope.drawTopDecor(mood: Mood) {
    when (mood) {
        Mood.MOSH -> {
            // devil horns
            drawPath(Art.hornL, Ink.darkRed)
            drawPath(Art.hornL, Ink.maroon, style = Pen.w6)
            drawPath(Art.hornR, Ink.darkRed)
            drawPath(Art.hornR, Ink.maroon, style = Pen.w6)
        }
        Mood.POGO -> {
            drawPath(Art.mohawk, Ink.green)
            drawPath(Art.mohawk, Ink.darkGreen, style = Pen.w6)
        }
        Mood.CUTE -> rotate(14f, Offset(640f, 372f)) {
            drawPath(Art.bowL, Ink.pink)
            drawPath(Art.bowR, Ink.pink)
            drawCircle(Ink.hotPink, 18f, Offset(640f, 372f))
        }
        Mood.SALSA -> {
            // hibiscus
            val flower = Offset(372f, 396f)
            for (a in 0 until 5) {
                rotate(a * 72f, flower) {
                    drawOval(Ink.red, Offset(flower.x - 24f, flower.y - 34f - 38f), Size(48f, 76f))
                }
            }
            drawCircle(Ink.yellow, 14f, flower)
        }
        Mood.GROOVE -> {
            // backwards cap
            drawPath(Art.capCrown, Ink.darkRed)
            drawPath(Art.capBrim, Ink.brick)
            drawCircle(Ink.brick, 10f, Offset(512f, 114f))
        }
        Mood.CHILL -> {
            // beret
            rotate(-8f, Offset(530f, 214f)) {
                drawOval(Ink.beret, Offset(530f - 150f, 214f - 44f), Size(300f, 88f))
            }
            drawRoundRect(Ink.beret, Offset(522f, 156f), Size(14f, 22f), CornerRadius(6f))
        }
        else -> Unit
    }
}

/** On the face: slides round the head with the eyes. */
private fun DrawScope.drawFaceDecor(mood: Mood) {
    when (mood) {
        Mood.CUTE -> drawPath(Art.lashes, Ink.navy, style = Pen.w7)
        // red lips
        Mood.SALSA -> drawPath(Art.smile, Ink.darkRed)
        Mood.CHILL -> {
            // round glasses and a bow tie
            drawCircle(Ink.navy, 50f, Offset(LX, LY), style = Pen.plain8)
            drawCircle(Ink.navy, 50f, Offset(RX, RY), style = Pen.plain8)
            drawPath(Art.glassesBridge, Ink.navy, style = Pen.plain8)
            drawPath(Art.bowTie, Ink.crimson)
            drawCircle(Ink.maroon, 12f, Offset(512f, 784f))
        }
        else -> Unit
    }
}
