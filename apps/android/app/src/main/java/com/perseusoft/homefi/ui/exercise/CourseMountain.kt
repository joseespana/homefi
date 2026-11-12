package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HotPoint
import com.perseusoft.homefi.core.ProfilePoint
import com.perseusoft.homefi.core.RideSegment
import com.perseusoft.homefi.core.SegmentKind
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.theme.LocalMotion

private val CARD_CORNER = 10.dp
private const val FRAME_NS = 33_000_000L // about 30 frames a second
private const val NS_PER_S = 1_000_000_000f

/**
 * The course as a mountain (docs/MOUNTAIN.md): [profile] is the core's elevation curve and [segments] the parts
 * (null for a saved ride's mini: neutral zone). With a [marker] it is the live ride: the rider's dot and beacon
 * sit on the ridge, the next different part is pinned. Without one the whole course is lit (plan, style card,
 * summary, My rides). The look and the summit labels are the rider's `courseLook` / `courseSummits` settings; the
 * beacon blinks only in full motion and while not [paused].
 */
@Composable
fun MountainProfile(
    profile: List<ProfilePoint>,
    marker: ProfilePoint?,
    modifier: Modifier = Modifier,
    segments: List<RideSegment>? = null,
    paused: Boolean = false,
    hotPoints: List<HotPoint> = emptyList(),
) {
    if (profile.size < 2) {
        Spacer(modifier)
        return
    }
    val app = LocalContext.current.homefi
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    val model = remember(profile, segments) { courseModelOf(profile, segments) }
    val measurer = rememberTextMeasurer()
    val kindNames = SegmentKind.entries.associateWith { stringResource(it.nameRes()) }

    val elapsed = marker?.tS?.toFloat()
    val current = elapsed?.let { partIndexAt(model.parts, it) }
    val pinPart = current?.let { nextDifferent(model.parts, it) }?.takeIf { it >= 0 }
    val pinText = pinPart?.let { i ->
        val part = model.parts[i]
        "${part.kind?.let(kindNames::get).orEmpty()} ${mmss(part.startS - (elapsed ?: 0f))}".trim()
    }.orEmpty()
    // the tags show only while the climbs' steps are on; thumbnails and the dot-only rider never draw the glyph
    val tags = if (settings.climbSteps > 0u) hotPoints else emptyList()
    val scene = rememberUpdatedState(CourseScene(elapsed, current, pinPart, pinText, settings.courseSummits, null, settings.courseRider, tags, paused))

    val animate = LocalMotion.current.moves && elapsed != null && !paused
    var clock by remember { mutableStateOf(0f) }
    LaunchedEffect(animate) {
        if (!animate) return@LaunchedEffect
        var start = -1L
        var last = 0L
        while (true) {
            withFrameNanos { now ->
                if (start < 0) start = now
                if (now - last >= FRAME_NS) {
                    last = now
                    clock = (now - start) / NS_PER_S
                }
            }
        }
    }
    val animated = rememberUpdatedState(animate)
    val total = mmss(model.totalS)
    val base = if (elapsed == null) {
        stringResource(R.string.ex_mountain_cd, total)
    } else {
        stringResource(R.string.ex_mountain_cd_live, mmss(elapsed), total, model.parts.getOrNull(current ?: 0)?.kind?.let(kindNames::get).orEmpty())
    }
    val description = if (tags.isEmpty()) base else "$base ${stringResource(R.string.ex_mountain_steps, tags.size)}"

    val look = settings.courseLook

    Spacer(
        modifier
            .clip(RoundedCornerShape(CARD_CORNER))
            .semantics { contentDescription = description }
            .drawWithCache {
                // rebuilt only when the size, the course or the look changes; the per-frame state is read in the draw block
                val layers = buildCourseLayers(model, look, size.width / density, size.height / density, density, measurer)
                onDrawWithContent {
                    drawCourse(layers, model, scene.value.withClock(if (animated.value) clock else null), measurer)
                }
            },
    )
}

private fun courseModelOf(profile: List<ProfilePoint>, segments: List<RideSegment>?): CourseModel {
    var at = 0f
    val parts = segments.orEmpty().map { seg ->
        CoursePart(seg.kind, at, seg.seconds.toFloat()).also { at += seg.seconds.toFloat() }
    }
    return CourseModel(profile.map { CoursePoint(it.tS.toFloat(), it.elevation) }, parts)
}
