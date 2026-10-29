package com.perseusoft.homefi.ui.player

import android.media.AudioManager
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.indication
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.VolumeDown
import androidx.compose.material.icons.automirrored.rounded.VolumeUp
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.AutoFixHigh
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableFloatState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.media3.common.C
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.LoudnessSettings
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.eqProfileFor
import com.perseusoft.homefi.data.SignalStyle
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.AudioOutput
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.EqTab
import com.perseusoft.homefi.player.NowPlaying
import com.perseusoft.homefi.player.OutputKind
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.eq.EqCurve
import com.perseusoft.homefi.ui.eq.formatDb
import com.perseusoft.homefi.ui.eq.sourceLabel
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.sound.label
import com.perseusoft.homefi.ui.sound.labelRes
import com.perseusoft.homefi.ui.sound.phrase
import com.perseusoft.homefi.ui.theme.Bevel
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.bevel
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import java.util.Locale
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.roundToInt
import kotlin.math.sin

// The signal chain: where the song comes from, what changes it on the way and where it goes,
// as a row of small stage nodes joined by flowing links. Only facts the player really has are
// shown: the file's (or stream's) format, the system mixer's rate for the phone's own outputs,
// the loudness gain, the equalizer profile and taste rule that play, and the output in use.

/** A stage of the chain (and the sheet its tap opens). */
private enum class Stage { SOURCE, RESAMPLE, LOUDNESS, EQ, OUTPUT, STATUS }

/** Bit-perfect, or what keeps it from being so. */
private enum class SignalStatus(val label: Int) {
    BIT_PERFECT(R.string.signal_bitperfect),
    RESAMPLED(R.string.signal_resampled),
    PROCESSED(R.string.signal_processed),
}

/** What is known about the song's audio before HomeFi touches it. */
private data class SourceFacts(
    val codec: String?,
    val bits: Int?,
    val rate: Int?,
    val channels: Int?,
    val bitrateKbps: Int?,
    val lossless: Boolean?,
    val path: String?,
    val tidalId: String?,
)

private data class ChainFacts(
    val source: SourceFacts,
    /** Source rate → output rate; the output rate is null when only "not one it accepts" is known. */
    val resample: Pair<Int, Int?>?,
    val outRate: Int?,
    /** [outRate] is the system mixer's (the phone's own speaker and jack). */
    val outRateFromMixer: Boolean,
    /** Loudness normalization's gain, when it changes anything. */
    val loudnessDb: Float?,
    val loudness: LoudnessSettings,
    /** The equalizer profile playing on this output, its shown name, and the preamp mode. */
    val eqProfile: EqProfile?,
    val eqName: String?,
    val autoPreamp: Boolean,
    val rule: TasteRule?,
    /** Any filter runs (the processor's own answer). */
    val active: Boolean,
    val output: AudioOutput?,
) {
    val eqShown: Boolean get() = eqName != null || rule != null || (active && loudnessDb == null)
    val status: SignalStatus get() = when {
        eqShown || loudnessDb != null || active -> SignalStatus.PROCESSED
        resample != null -> SignalStatus.RESAMPLED
        else -> SignalStatus.BIT_PERFECT
    }
}

/** Colours and shapes of the chain on one background (the app's skin, an LCD, a retro panel). */
@Immutable
class SignalLook(
    val fill: Color,
    val ink: Color,
    val inkVariant: Color,
    /** Untouched path. */
    val clean: Color,
    /** After a stage changed the samples. */
    val processed: Color,
    val shape: Shape,
    val font: FontFamily? = null,
    val bevel: Bevel? = null,
    val border: Color? = null,
    /** Behind the whole row (an LCD strip). */
    val strip: Color? = null,
    val caps: Boolean = false,
)

/** The look of the app's skin (light or dark). */
@Composable
fun skinSignalLook(): SignalLook {
    val skin = LocalSkin.current
    val cs = MaterialTheme.colorScheme
    val dark = cs.surface.luminance() < 0.5f
    return SignalLook(
        fill = if (skin.glass) cs.surfaceContainerHigh.copy(alpha = 0.72f) else cs.surfaceContainerHigh,
        ink = cs.onSurface,
        inkVariant = cs.onSurfaceVariant,
        clean = if (dark) CleanOnDark else CleanOnLight,
        processed = if (dark) AmberOnDark else AmberOnLight,
        shape = if (skin.roundCovers) RoundedCornerShape(50) else MaterialTheme.shapes.small,
        font = skin.numbers,
        bevel = skin.badgeBevel,
        border = skin.frame,
        caps = skin.capsLabels,
    )
}

private val CleanOnLight = Color(0xFF0F766E)
private val CleanOnDark = Color(0xFF2DD4BF)
private val AmberOnLight = Color(0xFFB45309)
private val AmberOnDark = Color(0xFFFBBF24)

/** Green segments on black, like the amp's display. */
internal val LcdSignalLook = SignalLook(
    fill = Color(0xFF061206),
    ink = Color(0xFF00E000),
    inkVariant = Color(0xFF00A000),
    clean = Color(0xFF00E000),
    processed = Color(0xFFFFB000),
    shape = RectangleShape,
    font = FontFamily.Monospace,
    border = Color(0xFF0E3A0E),
    strip = Color(0xFF000000),
    caps = true,
)

/** White on the night-blue glass of Luna. */
internal val LunaSignalLook = SignalLook(
    fill = Color(0xFF1D2D5C),
    ink = Color.White,
    inkVariant = Color(0xFF9EC5FF),
    clean = Color(0xFF8BE38F),
    processed = Color(0xFFFFC857),
    shape = RoundedCornerShape(50),
    border = Color(0xFF5C7CC0),
)

/** Dark ink on the pale aluminum LCD. */
internal val AluminumSignalLook = SignalLook(
    fill = Color(0xFFE4E7D2),
    ink = Color(0xFF1E1E1E),
    inkVariant = Color(0xFF4A4A4A),
    clean = Color(0xFF2E7D32),
    processed = Color(0xFFB26A00),
    shape = RoundedCornerShape(4.dp),
    border = Color(0xFF8C8C8C),
)

/** The light grey panels of Web 2000. */
internal val Web2000SignalLook = SignalLook(
    fill = Color(0xFFE6E6E6),
    ink = Color(0xFF222222),
    inkVariant = Color(0xFF555555),
    clean = Color(0xFF5E8F1E),
    processed = Color(0xFFC77700),
    shape = RoundedCornerShape(10.dp),
    border = Color(0xFFB5B5B5),
)

/**
 * Whether the player shows the chain (else today's quality and EQ badges): its setting is on,
 * the screen is tall enough and the song plays on this phone (a guest hears the hub's chain).
 */
@Composable
fun signalChainShown(remote: Boolean): Boolean {
    val settings by LocalContext.current.homefi.settings.settings.collectAsStateWithLifecycle()
    return settings.signalChain && !remote && !LocalPlayerCompact.current
}

/** A node of the row. */
private class Node(
    val stage: Stage,
    val icon: ImageVector,
    val caption: String,
    val chip: String?,
    val text: String?,
    val extra: String?,
    /** For TalkBack: the stage and all its facts. */
    val description: String,
    /** The stage changes the samples (amber from here on). */
    val processes: Boolean = false,
    /** The stage converts the rate (denser dots from here on). */
    val resamples: Boolean = false,
)

/**
 * The chain for [current]: source → resample → loudness → equalizer → output, then a status pill.
 * The links flow while the player plays (when motion and the "Animate the signal" setting allow).
 */
@Composable
fun SignalChain(
    current: NowPlaying?,
    modifier: Modifier = Modifier,
    look: SignalLook = skinSignalLook(),
    centered: Boolean = true,
) {
    current ?: return
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val source = playerSource()
    // only play / pause matters here, not every position tick
    val playing by remember(source) { source.state.map { it.isPlaying }.distinctUntilChanged() }.collectAsStateWithLifecycle(false)
    val facts = rememberChainFacts(app, current)
    val detailed = settings.signalStyle == SignalStyle.DETAILED
    val animate = settings.signalAnimate && LocalMotion.current.moves && playing
    var open by remember { mutableStateOf<Stage?>(null) }
    var picker by remember { mutableStateOf(false) }

    val nodes = chainNodes(facts)
    val phase = rememberSignalPhase(animate)
    Box(
        modifier
            .fillMaxWidth()
            .then(if (look.strip != null) Modifier.background(look.strip).border(1.dp, look.border ?: look.strip) else Modifier),
        contentAlignment = if (centered) Alignment.Center else Alignment.CenterStart,
    ) {
        Row(
            Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = if (look.strip != null) 4.dp else 0.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            var tainted = false
            var dense = false
            nodes.forEachIndexed { i, n ->
                if (i > 0) SignalLink(if (tainted) look.processed else look.clean, dense, phase)
                tainted = tainted || n.processes
                dense = dense || n.resamples
                val accent = when {
                    n.processes -> look.processed
                    n.stage == Stage.OUTPUT && tainted -> look.processed
                    else -> look.clean
                }
                StageNode(n, look, accent, detailed) { open = n.stage }
            }
            Spacer(Modifier.width(6.dp))
            StatusPill(facts.status, look, detailed) { open = Stage.STATUS }
        }
    }

    open?.let { stage ->
        StageSheet(
            stage = stage,
            facts = facts,
            onDismiss = { open = null },
            onOpenEqualizer = {
                open = null
                app.equalizer.open(EqPanel.EQUALIZER, if (facts.rule != null && facts.eqName == null) EqTab.TASTE else null)
            },
            onChangeOutput = {
                open = null
                picker = true
            },
        )
    }
    if (picker) OutputPicker(onDismiss = { picker = false })
}

/**
 * One clock for the whole row: it moves only while [animate], and only the links' draw phase
 * reads it, so nothing recomposes per frame. Paused, the dots stay where they were.
 */
@Composable
private fun rememberSignalPhase(animate: Boolean): MutableFloatState {
    val phase = remember { mutableFloatStateOf(0f) }
    LaunchedEffect(animate) {
        if (!animate) return@LaunchedEffect
        var last = 0L
        while (true) {
            withFrameNanos { t ->
                if (last != 0L) phase.floatValue = (phase.floatValue + (t - last) / LOOP_NANOS) % 1f
                last = t
            }
        }
    }
    return phase
}

private const val LOOP_NANOS = 1_600_000_000f

/** A small wave with dots riding it left to right; after a resampler the dots get denser and smaller. */
@Composable
private fun SignalLink(color: Color, dense: Boolean, phase: MutableFloatState) {
    Box(
        Modifier
            .width(22.dp)
            .height(16.dp)
            .drawWithCache {
                val path = Path()
                val wave = 11.dp.toPx()
                val amp = size.height * 0.16f
                val mid = size.height / 2
                val gap = if (dense) 4.dp.toPx() else 7.dp.toPx()
                val radius = if (dense) 1.1.dp.toPx() else 1.7.dp.toPx()
                val stroke = Stroke(1.dp.toPx(), cap = StrokeCap.Round)
                fun y(x: Float, p: Float) = mid + amp * sin(2 * PI * (x / wave - p)).toFloat()
                onDrawBehind {
                    val p = phase.floatValue // read here: only the drawing follows the clock
                    path.reset()
                    var x = 0f
                    path.moveTo(0f, y(0f, p))
                    while (x < size.width) {
                        x += 1.5f
                        path.lineTo(x, y(x, p))
                    }
                    drawPath(path, color.copy(alpha = 0.35f), style = stroke)
                    var d = (p * 2 * gap) % gap
                    while (d < size.width) {
                        // fade in and out at the ends so the dots seem to come out of the nodes
                        val edge = (minOf(d, size.width - d) / (size.width * 0.25f)).coerceIn(0f, 1f)
                        drawCircle(color.copy(alpha = 0.35f + 0.65f * edge), radius, Offset(d, y(d, p)))
                        d += gap
                    }
                }
            },
    )
}

@Composable
private fun StageNode(n: Node, look: SignalLook, accent: Color, detailed: Boolean, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val haptics = haptics()
    val details = stringResource(R.string.signal_details)
    // the touch target is 48 dp tall; the node drawn inside it is smaller
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .semantics(mergeDescendants = true) { contentDescription = n.description }
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClickLabel = details) {
                haptics.tick()
                onClick()
            },
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .clip(look.shape)
                .background(look.fill)
                .then(if (look.bevel != null) Modifier.bevel(look.bevel) else if (look.border != null) Modifier.border(1.dp, look.border, look.shape) else Modifier)
                .indication(interaction, LocalIndication.current)
                .padding(horizontal = 7.dp, vertical = if (detailed) 3.dp else 4.dp),
        ) {
            if (detailed) {
                Text(
                    if (look.caps) n.caption.uppercase() else n.caption,
                    color = look.inkVariant,
                    fontSize = 9.sp,
                    lineHeight = 10.sp,
                    fontFamily = look.font,
                    maxLines = 1,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                Icon(n.icon, null, Modifier.size(14.dp), tint = accent)
                n.chip?.let { SmallChip(it, accent, look) }
                n.text?.let {
                    Text(
                        it,
                        color = look.ink,
                        fontSize = 11.sp,
                        lineHeight = 13.sp,
                        fontFamily = look.font,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.widthIn(max = 112.dp),
                    )
                }
                n.extra?.let { SmallChip(it, look.inkVariant, look) }
            }
        }
    }
}

@Composable
private fun SmallChip(text: String, color: Color, look: SignalLook) {
    Text(
        text,
        color = color,
        fontSize = 9.sp,
        lineHeight = 11.sp,
        fontWeight = FontWeight.Bold,
        fontFamily = look.font,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier
            .widthIn(max = 88.dp)
            .clip(if (look.shape == RectangleShape) RectangleShape else RoundedCornerShape(4.dp))
            .background(color.copy(alpha = 0.16f))
            .padding(horizontal = 4.dp, vertical = 1.dp),
    )
}

@Composable
private fun StatusPill(status: SignalStatus, look: SignalLook, detailed: Boolean, onClick: () -> Unit) {
    val color = if (status == SignalStatus.BIT_PERFECT) look.clean else look.processed
    val label = stringResource(status.label)
    val description = stringResource(R.string.signal_cd_status, label)
    val details = stringResource(R.string.signal_details)
    val interaction = remember { MutableInteractionSource() }
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .semantics(mergeDescendants = true) { contentDescription = description }
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClickLabel = details, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier
                .clip(if (look.shape == RectangleShape) RectangleShape else RoundedCornerShape(50))
                .background(color.copy(alpha = 0.16f))
                .border(1.dp, color.copy(alpha = 0.5f), if (look.shape == RectangleShape) RectangleShape else RoundedCornerShape(50))
                .indication(interaction, LocalIndication.current)
                .padding(horizontal = 8.dp, vertical = if (detailed) 6.dp else 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                when (status) {
                    SignalStatus.BIT_PERFECT -> Icons.Rounded.Verified
                    SignalStatus.RESAMPLED -> Icons.Rounded.SwapHoriz
                    SignalStatus.PROCESSED -> Icons.Rounded.AutoFixHigh
                },
                null,
                Modifier.size(13.dp),
                tint = color,
            )
            Spacer(Modifier.width(4.dp))
            Text(
                if (look.caps) label.uppercase() else label,
                color = color,
                fontSize = 11.sp,
                lineHeight = 13.sp,
                fontWeight = FontWeight.Bold,
                fontFamily = look.font,
                maxLines = 1,
            )
        }
    }
}

// ------------------------------------------------------------------ facts

@Composable
private fun rememberChainFacts(app: HomefiApplication, current: NowPlaying): ChainFacts {
    val now by app.sound.now.collectAsStateWithLifecycle()
    val output by app.outputs.current.collectAsStateWithLifecycle()
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val outputId by app.equalizer.outputId.collectAsStateWithLifecycle()
    val loudness by app.settings.loudness.collectAsStateWithLifecycle()
    val source = remember(current.mediaId, current.quality) { sourceFacts(app, current) }
    val mixerRate = remember {
        app.getSystemService(AudioManager::class.java)?.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toIntOrNull()?.takeIf { it > 0 }
    }
    val profile = remember(eq, outputId, now.eqProfile) {
        if (now.eqProfile == null) null else runCatching { eqProfileFor(eq, outputId) }.getOrNull()
    }
    // the phone's own speaker and jack go through the primary mixer at its rate; other
    // outputs only tell when they accept a single rate
    val fromMixer = output == null || output?.kind == OutputKind.SPEAKER || output?.kind == OutputKind.WIRED
    val outRate = if (fromMixer) mixerRate else output?.sampleRates?.singleOrNull()
    val src = source.rate
    val resample = when {
        src == null -> null
        outRate != null -> if (outRate != src) src to outRate else null
        output?.sampleRates?.isNotEmpty() == true && src !in output!!.sampleRates -> src to null
        else -> null
    }
    return ChainFacts(
        source = source,
        resample = resample,
        outRate = outRate,
        outRateFromMixer = fromMixer && outRate != null,
        loudnessDb = now.loudnessDb.takeIf { abs(it) >= 0.05f },
        loudness = loudness,
        eqProfile = profile,
        eqName = now.eqProfile,
        autoPreamp = eq.autoPreamp,
        rule = now.rule,
        active = now.active,
        output = output,
    )
}

/** The file's tags for a library track; else the decoder's format and the player's quality line. */
private fun sourceFacts(app: HomefiApplication, current: NowPlaying): SourceFacts {
    val id = current.mediaId
    val path = id.takeIf(::isFilePath)
    path?.let(app.library::find)?.let { t ->
        return SourceFacts(
            codec = t.format.ifBlank { null },
            bits = t.bitDepth?.toInt()?.takeIf { it > 0 },
            rate = t.sampleRate?.toInt()?.takeIf { it > 0 },
            channels = t.channels?.toInt()?.takeIf { it > 0 },
            bitrateKbps = t.bitrateKbps?.toInt()?.takeIf { it > 0 },
            lossless = t.lossless,
            path = t.path,
            tidalId = null,
        )
    }
    val format = runCatching {
        app.player.player?.currentTracks?.groups
            ?.firstOrNull { it.type == C.TRACK_TYPE_AUDIO && it.isSelected }
            ?.let { g -> (0 until g.length).firstOrNull(g::isTrackSelected)?.let(g::getTrackFormat) }
    }.getOrNull()
    val q = current.quality.orEmpty()
    val codec = q.substringBefore('·').trim().ifBlank { null }
    val rate = format?.sampleRate?.takeIf { it > 0 }
        ?: Regex("""([\d.]+)\s*kHz""").find(q)?.groupValues?.get(1)?.toDoubleOrNull()?.let { (it * 1000).roundToInt() }
    val kbps = (format?.averageBitrate?.takeIf { it > 0 } ?: format?.bitrate?.takeIf { it > 0 })?.let { it / 1000 }
        ?: Regex("""(\d+)\s*kbps""").find(q)?.groupValues?.get(1)?.toIntOrNull()
    return SourceFacts(
        codec = codec,
        bits = Regex("""(\d+)-bit""").find(q)?.groupValues?.get(1)?.toIntOrNull(),
        rate = rate,
        channels = format?.channelCount?.takeIf { it > 0 },
        bitrateKbps = kbps,
        lossless = codec?.uppercase(Locale.ROOT)?.let { it in LOSSLESS },
        path = path,
        tidalId = tidalId(id),
    )
}

private val LOSSLESS = setOf("FLAC", "ALAC", "WAV", "AIFF", "APE", "WAVPACK", "WV", "DSD", "DSF")

/** `96`, `44.1` */
private fun khz(rate: Int): String =
    if (rate % 1000 == 0) "${rate / 1000}" else String.format(Locale.ROOT, "%.1f", rate / 1000.0)

/** `−4.2 dB` with a real minus sign. */
@Composable
private fun db(value: Float): String = stringResource(R.string.eq_db, formatDb(value).replace('-', '−'))

@Composable
private fun channelsText(n: Int): String = when (n) {
    1 -> stringResource(R.string.signal_mono)
    2 -> stringResource(R.string.signal_stereo)
    else -> stringResource(R.string.signal_channels_n, n)
}

@Composable
private fun tierText(s: SourceFacts): String? = when {
    s.lossless == null -> null
    !s.lossless -> stringResource(R.string.signal_tier_lossy)
    (s.bits ?: 16) > 16 || (s.rate ?: 44_100) > 48_000 -> stringResource(R.string.signal_tier_hires)
    else -> stringResource(R.string.signal_tier_lossless)
}

@Composable
private fun outputName(o: AudioOutput?): String =
    o?.name?.ifBlank { null } ?: stringResource((o?.kind ?: OutputKind.SPEAKER).label())

@Composable
private fun chainNodes(f: ChainFacts): List<Node> {
    val s = f.source
    val tidal = s.tidalId != null
    val nodes = mutableListOf<Node>()

    val sourceCaption = stringResource(R.string.signal_stage_source)
    val sourceText = when {
        s.bits != null && s.rate != null -> "${s.bits}/${khz(s.rate)}"
        s.rate != null -> "${khz(s.rate)} kHz"
        s.bitrateKbps != null -> stringResource(R.string.signal_kbps, s.bitrateKbps)
        else -> null
    }
    val sourceFacts = listOfNotNull(
        s.codec,
        s.bits?.let { stringResource(R.string.signal_bits, it) },
        s.rate?.let { "${khz(it)} kHz" },
        s.channels?.let { channelsText(it) },
        tierText(s),
        stringResource(if (tidal) R.string.player_source_tidal else R.string.player_source_local),
    ).joinToString(", ")
    nodes += Node(
        Stage.SOURCE,
        if (tidal) Icons.Rounded.Cloud else Icons.Rounded.AudioFile,
        sourceCaption,
        chip = s.codec ?: if (tidal) "Tidal" else null,
        text = sourceText,
        extra = null,
        description = stringResource(R.string.signal_cd_stage, sourceCaption, sourceFacts),
    )

    f.resample?.let { (from, to) ->
        val caption = stringResource(R.string.signal_stage_resample)
        val text = if (to != null) "${khz(from)}→${khz(to)} kHz" else "${khz(from)} kHz→"
        nodes += Node(
            Stage.RESAMPLE, Icons.Rounded.SwapHoriz, caption, chip = null, text = text, extra = null,
            description = stringResource(R.string.signal_cd_stage, caption, text),
            resamples = true,
        )
    }

    f.loudnessDb?.let { gain ->
        val caption = stringResource(R.string.signal_stage_loudness)
        val text = db(gain)
        nodes += Node(
            Stage.LOUDNESS,
            if (gain < 0) Icons.AutoMirrored.Rounded.VolumeDown else Icons.AutoMirrored.Rounded.VolumeUp,
            caption, chip = null, text = text, extra = null,
            description = stringResource(R.string.signal_cd_stage, caption, "${stringResource(f.loudness.mode.labelRes())}, $text"),
            processes = true,
        )
    }

    if (f.eqShown) {
        val caption = stringResource(R.string.signal_stage_eq)
        val profile = f.eqName?.let { it.ifBlank { stringResource(R.string.eq_custom) } }
        val taste = f.rule?.let { r -> r.label.ifBlank { r.scope.name() }.ifBlank { null } }
        val text = profile ?: taste ?: stringResource(R.string.eq_custom)
        val extra = taste.takeIf { profile != null }
        nodes += Node(
            Stage.EQ,
            if (profile == null && taste != null) Icons.Rounded.Tune else Icons.Rounded.GraphicEq,
            caption, chip = null, text = text, extra = extra,
            description = stringResource(R.string.signal_cd_stage, caption, listOfNotNull(profile, taste).ifEmpty { listOf(text) }.joinToString(" + ")),
            processes = true,
        )
    }

    val o = f.output
    val caption = stringResource(R.string.signal_stage_output)
    val name = outputName(o)
    val rateText = f.outRate?.let { "${khz(it)} kHz" }
    nodes += Node(
        Stage.OUTPUT,
        (o?.kind ?: OutputKind.SPEAKER).icon(),
        caption, chip = null, text = name, extra = rateText,
        description = stringResource(
            R.string.signal_cd_stage,
            caption,
            listOfNotNull(name, stringResource((o?.kind ?: OutputKind.SPEAKER).label()).takeIf { it != name }, rateText).joinToString(", "),
        ),
    )
    return nodes
}

// ------------------------------------------------------------------ sheets

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun StageSheet(stage: Stage, facts: ChainFacts, onDismiss: () -> Unit, onOpenEqualizer: () -> Unit, onChangeOutput: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(start = 16.dp, end = 16.dp, bottom = 24.dp),
        ) {
            when (stage) {
                Stage.SOURCE -> SourceDetails(facts.source)
                Stage.RESAMPLE -> ResampleDetails(facts)
                Stage.LOUDNESS -> LoudnessDetails(facts)
                Stage.EQ -> EqDetails(facts, onOpenEqualizer)
                Stage.OUTPUT -> OutputDetails(facts, onChangeOutput)
                Stage.STATUS -> StatusDetails(facts.status)
            }
        }
    }
}

@Composable
private fun SheetTitle(icon: ImageVector, text: String, tint: Color = MaterialTheme.colorScheme.primary) {
    Row(Modifier.padding(bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, Modifier.size(22.dp), tint = tint)
        Spacer(Modifier.width(10.dp))
        Text(text, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun Fact(label: Int, value: String?) {
    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.Top) {
        Text(stringResource(label), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(0.4f))
        Text(
            value ?: stringResource(R.string.signal_unknown),
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.weight(0.6f),
        )
    }
}

@Composable
private fun Note(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(vertical = 6.dp),
    )
}

@Composable
private fun SourceDetails(s: SourceFacts) {
    SheetTitle(if (s.tidalId != null) Icons.Rounded.Cloud else Icons.Rounded.AudioFile, stringResource(R.string.signal_stage_source))
    Fact(R.string.signal_codec, s.codec)
    if (s.lossless != false || s.bits != null) Fact(R.string.signal_bit_depth, s.bits?.let { stringResource(R.string.signal_bits, it) })
    Fact(R.string.signal_sample_rate, s.rate?.let { "${khz(it)} kHz" })
    Fact(R.string.signal_channels, s.channels?.let { "${channelsText(it)} ($it)" })
    if (s.lossless == false || s.bitrateKbps != null) Fact(R.string.signal_bitrate, s.bitrateKbps?.let { stringResource(R.string.signal_kbps, it) })
    Fact(R.string.signal_tier, tierText(s))
    when {
        s.tidalId != null -> Fact(R.string.signal_tidal_track, s.tidalId)
        s.path != null -> Fact(R.string.signal_file, s.path)
    }
}

@Composable
private fun ResampleDetails(f: ChainFacts) {
    val (from, to) = f.resample ?: return
    SheetTitle(Icons.Rounded.SwapHoriz, stringResource(R.string.signal_stage_resample), AmberIn())
    Fact(R.string.signal_from, "${khz(from)} kHz")
    Fact(R.string.signal_to, to?.let { "${khz(it)} kHz" } ?: stringResource(R.string.signal_device_rate))
    if (to != null) Fact(R.string.signal_ratio, String.format(Locale.ROOT, "×%.4f", to.toDouble() / from))
    Fact(R.string.signal_algorithm, stringResource(R.string.signal_algorithm_android))
    Note(stringResource(R.string.signal_resample_why, to?.let { "${khz(it)} kHz" } ?: stringResource(R.string.signal_device_rate)))
}

@Composable
private fun LoudnessDetails(f: ChainFacts) {
    val gain = f.loudnessDb ?: 0f
    SheetTitle(if (gain < 0) Icons.AutoMirrored.Rounded.VolumeDown else Icons.AutoMirrored.Rounded.VolumeUp, stringResource(R.string.sound_loudness), AmberIn())
    Fact(R.string.signal_mode, stringResource(f.loudness.mode.labelRes()))
    Fact(R.string.signal_target, stringResource(R.string.sound_loudness_lufs, formatDb(f.loudness.targetLufs).removePrefix("+").replace('-', '−')))
    Fact(R.string.signal_gain, db(gain))
    Fact(R.string.signal_clipping_guard, stringResource(if (f.loudness.preventClipping) R.string.signal_on else R.string.signal_off))
}

@Composable
private fun EqDetails(f: ChainFacts, onOpen: () -> Unit) {
    SheetTitle(Icons.Rounded.GraphicEq, stringResource(R.string.signal_stage_eq), AmberIn())
    val p = f.eqProfile
    if (f.eqName != null) {
        Fact(R.string.signal_profile, f.eqName.ifBlank { stringResource(R.string.eq_custom) })
        if (p != null) {
            Fact(R.string.signal_profile_source, p.sourceLabel() ?: stringResource(R.string.eq_source_manual))
            EqCurve(p, Modifier.fillMaxWidth().height(110.dp).padding(vertical = 6.dp), showPoints = false)
            val preamp = db(p.preampDb)
            Fact(R.string.eq_preamp, if (f.autoPreamp) stringResource(R.string.signal_preamp_auto, preamp) else preamp)
            Fact(R.string.signal_bands, "${p.bands.count { it.enabled }}")
        }
    }
    val r = f.rule
    if (r != null) {
        Fact(R.string.signal_taste, listOf(r.label.ifBlank { r.scope.phrase() }, LocalContext.current.homefi.sound.ruleWords(r)).joinToString(" · "))
        if (r.bands.isNotEmpty()) Fact(R.string.signal_taste_bands, stringResource(R.string.signal_taste_bands_value, r.bands.count { it.band.enabled }, r.bands.size))
        Fact(R.string.signal_profile_source, r.createdBy.label())
    } else {
        Fact(R.string.signal_taste, stringResource(R.string.signal_taste_none))
    }
    Spacer(Modifier.height(8.dp))
    FilledTonalButton(onClick = onOpen, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.eq_open)) }
}

@Composable
private fun OutputDetails(f: ChainFacts, onChange: () -> Unit) {
    val o = f.output
    val kind = o?.kind ?: OutputKind.SPEAKER
    SheetTitle(kind.icon(), stringResource(R.string.signal_stage_output))
    Fact(R.string.signal_name, outputName(o))
    Fact(R.string.signal_type, stringResource(kind.label()))
    Fact(
        R.string.signal_rate,
        f.outRate?.let { r -> "${khz(r)} kHz".let { if (f.outRateFromMixer) stringResource(R.string.signal_rate_mixer, it) else it } },
    )
    o?.capabilities()?.takeIf { it.isNotBlank() }?.let { Fact(R.string.signal_supports, it) }
    Fact(R.string.signal_channels, o?.maxChannels?.takeIf { it > 0 }?.let { channelsText(it) })
    Spacer(Modifier.height(8.dp))
    FilledTonalButton(onClick = onChange, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.signal_change_output)) }
}

@Composable
private fun StatusDetails(status: SignalStatus) {
    val color = if (status == SignalStatus.BIT_PERFECT) CleanIn() else AmberIn()
    SheetTitle(
        when (status) {
            SignalStatus.BIT_PERFECT -> Icons.Rounded.Verified
            SignalStatus.RESAMPLED -> Icons.Rounded.SwapHoriz
            SignalStatus.PROCESSED -> Icons.Rounded.AutoFixHigh
        },
        stringResource(R.string.signal_about_title),
        color,
    )
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 6.dp)) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
        Spacer(Modifier.width(8.dp))
        Text(stringResource(R.string.signal_about_now, stringResource(status.label)), style = MaterialTheme.typography.titleSmall, color = color)
    }
    Note(stringResource(R.string.signal_about_bitperfect))
    Note(stringResource(R.string.signal_about_resampled))
    Note(stringResource(R.string.signal_about_processed))
    Note(stringResource(R.string.signal_about_android))
}

/** The sheet's own colours follow the app's theme, not the player's panel. */
@Composable
private fun CleanIn(): Color = if (MaterialTheme.colorScheme.surface.luminance() < 0.5f) CleanOnDark else CleanOnLight

@Composable
private fun AmberIn(): Color = if (MaterialTheme.colorScheme.surface.luminance() < 0.5f) AmberOnDark else AmberOnLight
