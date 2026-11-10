package com.perseusoft.homefi.tv.train

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import com.perseusoft.homefi.core.Popularity
import com.perseusoft.homefi.core.exerciseDecades
import com.perseusoft.homefi.tv.TvRow
import com.perseusoft.homefi.ui.exercise.Genre
import com.perseusoft.homefi.ui.exercise.decadeLabel
import com.perseusoft.homefi.ui.exercise.labelRes
import com.perseusoft.homefi.ui.exercise.rideLine
import com.perseusoft.homefi.ui.exercise.toggleDecade
import com.perseusoft.homefi.ui.exercise.toggleGenre
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.RideUi
import com.perseusoft.homefi.tv.RideWebView
import com.perseusoft.homefi.tv.TvConfirmDialog
import com.perseusoft.homefi.tv.TvPill
import com.perseusoft.homefi.tv.TvType
import com.perseusoft.homefi.tv.LocalTvLook
import com.perseusoft.homefi.tv.tvInitialFocus
import com.perseusoft.homefi.tv.tvSafePadding
import com.perseusoft.homefi.core.RideProgress
import com.perseusoft.homefi.core.exerciseHealthNotice
import com.perseusoft.homefi.core.exercisePresets
import com.perseusoft.homefi.tv.TvSettingLine
import com.perseusoft.homefi.tv.ValueText
import com.perseusoft.homefi.tv.cycleIndex
import com.perseusoft.homefi.tv.nearestIndex
import com.perseusoft.homefi.tv.tvLeftToDrawer
import com.perseusoft.homefi.ui.exercise.MountainProfile
import com.perseusoft.homefi.ui.playlists.languageTag

/** Minutes the length row steps through. */
private val MINUTES = (10..90 step 5).toList()
private val FORM_WIDTH = 640.dp
private val CHIP_HEIGHT = 34.dp

/** Saved rides listed on the TV (the phone has the rest). */
private const val FIRST_RIDES = 6

/**
 * Setting up a ride on this TV, made for the D-pad: the length and the style as value rows (LEFT / RIGHT change
 * them), "Build my ride", then the plan with Start. Everything is the phone's [RideSession]: the form, the
 * build, the plan, "start". BACK steps out of a plan, cancels a build, or leaves ([onExit]) from the first page.
 * The ride itself is [TvRideOverlay]. Genres, fame, era and saved rides are here too; liked and avoided artists and the AI chat stay on the phone.
 */
@Composable
fun TvRideSetup(entryKey: Int, onExit: () -> Unit) {
    val app = LocalContext.current.homefi
    val session = app.ride
    val ui by session.ui.collectAsStateWithLifecycle()
    val form by session.form.collectAsStateWithLifecycle()
    val settings by app.settings.exercise.collectAsStateWithLifecycle()
    val language = languageTag()
    val presets = remember(language) { exercisePresets(language) }
    val decades = remember { exerciseDecades() }
    val rides by app.rides.list.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { app.rides.refresh() }
    var notice by remember { mutableStateOf(false) }
    BackHandler {
        when (ui) {
            is RideUi.Building -> session.cancelBuild()
            is RideUi.Preview, is RideUi.Failed -> session.backToSetup()
            else -> onExit()
        }
    }
    val look = LocalTvLook.current
    Column(
        Modifier.fillMaxSize().padding(tvSafePadding()).padding(start = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(stringResource(R.string.tv_train_here), style = TvType.title, color = look.ink, maxLines = 1)
        when (val state = ui) {
            RideUi.Setup -> Column(
                Modifier.width(FORM_WIDTH).weight(1f).verticalScroll(rememberScrollState()).padding(vertical = 6.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                val minutesIndex = nearestIndex(MINUTES, form.minutes)
                ValueLine(
                    stringResource(R.string.ex_duration), "${MINUTES[minutesIndex]} min", MINUTES.size, minutesIndex,
                    Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(),
                ) { i -> session.updateForm { it.copy(minutes = MINUTES[i]) } }
                val styleIndex = presets.indexOfFirst { it.id == form.style }.coerceAtLeast(0)
                ValueLine(
                    stringResource(R.string.ex_style), presets.getOrNull(styleIndex)?.name.orEmpty(), presets.size, styleIndex,
                    Modifier.tvLeftToDrawer(),
                ) { i -> presets.getOrNull(i)?.let { p -> session.updateForm { it.copy(style = p.id) } } }
                val popularity = settings.taste.popularity
                ValueLine(
                    stringResource(R.string.ex_taste_popularity), stringResource(popularity.labelRes()), Popularity.entries.size,
                    Popularity.entries.indexOf(popularity), Modifier.tvLeftToDrawer(),
                ) { i -> session.updateTaste { it.copy(popularity = Popularity.entries[i]) } }
                ChipGroup(stringResource(R.string.ex_music)) {
                    Genre.entries.forEachIndexed { i, genre ->
                        TvPill(
                            stringResource(genre.label), { session.updateForm { it.copy(genres = toggleGenre(it.genres, genre.token)) } },
                            Modifier.chipEdge(i), selected = genre.token in form.genres, height = CHIP_HEIGHT,
                        )
                    }
                }
                ChipGroup(stringResource(R.string.ex_taste_era)) {
                    TvPill(
                        stringResource(R.string.ex_era_any), { session.updateTaste { it.copy(eras = emptyList()) } },
                        Modifier.chipEdge(0), selected = settings.taste.eras.isEmpty(), height = CHIP_HEIGHT,
                    )
                    decades.forEach { decade ->
                        TvPill(
                            decadeLabel(decade), { session.updateTaste { it.copy(eras = toggleDecade(it.eras, decade)) } },
                            selected = decade in settings.taste.eras, height = CHIP_HEIGHT,
                        )
                    }
                }
                TvPill(
                    stringResource(R.string.ex_build), { if (settings.healthNoticeSeen) session.build() else notice = true },
                    Modifier.padding(top = 8.dp).tvLeftToDrawer(), icon = Icons.Rounded.AutoAwesome, primary = true, height = 40.dp,
                )
                if (rides.isNotEmpty()) {
                    Text(stringResource(R.string.rides_title), style = TvType.section, color = look.ink, modifier = Modifier.padding(top = 8.dp))
                    rides.take(FIRST_RIDES).forEach { ride ->
                        key(ride.id) {
                            TvRow(
                                ride.name, rideLine(ride), { session.rideAgain(ride.id) }, Modifier.tvLeftToDrawer(),
                                coverIcon = Icons.Rounded.PlayArrow,
                            )
                        }
                    }
                }
                Text(stringResource(R.string.tv_train_phone_more), style = TvType.secondary, color = look.ink3)
            }
            is RideUi.Building -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.ex_building), style = TvType.section, color = look.ink)
                Text(buildStep(state), style = TvType.body, color = look.ink2)
                TvPill(stringResource(R.string.ex_cancel), session::cancelBuild, Modifier.tvInitialFocus().tvLeftToDrawer(), height = 36.dp)
            }
            is RideUi.Preview -> RidePlan(state, session)
            is RideUi.Failed -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(state.message, style = TvType.body, color = look.error)
                TvPill(stringResource(R.string.ex_change), session::backToSetup, Modifier.tvInitialFocus().tvLeftToDrawer(), height = 36.dp)
            }
            else -> Unit
        }
    }
    if (notice) {
        TvConfirmDialog(
            title = stringResource(R.string.ex_health_title),
            text = exerciseHealthNotice(language),
            confirmLabel = stringResource(R.string.ex_health_ok),
            onConfirm = {
                app.settings.setExercise(settings.copy(healthNoticeSeen = true))
                session.build()
            },
            onDismiss = { notice = false },
        )
    }
}

/** A titled, horizontally scrolling row of toggle chips; focus brings the chip into view. */
@Composable
private fun ChipGroup(title: String, chips: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    val look = LocalTvLook.current
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = TvType.secondary, color = look.ink2, modifier = Modifier.padding(start = 4.dp))
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 4.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            content = chips,
        )
    }
}

/** LEFT from the first chip of a row goes to the drawer. */
private fun Modifier.chipEdge(index: Int): Modifier = if (index == 0) tvLeftToDrawer() else this

/** What the build is doing, as the core reports it (the same words as the phone's build screen). */
@Composable
private fun buildStep(state: RideUi.Building): String = when (val progress = state.progress) {
    null -> stringResource(R.string.ex_step_starting)
    is RideProgress.Searching ->
        if (progress.withAi) stringResource(R.string.ex_step_ai, state.aiName.orEmpty()) else stringResource(R.string.ex_step_search)
    is RideProgress.Resolving -> stringResource(R.string.ex_step_resolving, progress.done.toInt(), progress.total.toInt())
    is RideProgress.Analyzing -> stringResource(R.string.ex_step_analyzing, progress.done.toInt(), progress.total.toInt())
    RideProgress.Matching -> stringResource(R.string.ex_step_matching)
}

/** A row with a value that LEFT / RIGHT change (wrapping), like Settings' value rows. */
@Composable
private fun ValueLine(title: String, value: String, count: Int, index: Int, modifier: Modifier, onIndex: (Int) -> Unit) {
    TvSettingLine(
        title, null, true,
        modifier.onPreviewKeyEvent { event ->
            val step = when (event.key) {
                Key.DirectionLeft -> -1
                Key.DirectionRight -> 1
                else -> 0
            }
            if (step == 0) return@onPreviewKeyEvent false
            if (event.type == KeyEventType.KeyDown) onIndex(cycleIndex(index, count, step))
            true
        },
        onClick = { onIndex(cycleIndex(index, count, 1)) },
    ) { focused -> ValueText(value, focused, arrows = true) }
}

/** The plan: name, size, its mountain, Start. */
@Composable
private fun RidePlan(state: RideUi.Preview, session: com.perseusoft.homefi.player.exercise.RideSession) {
    val look = LocalTvLook.current
    val plan = state.built.plan
    val segments = plan.workout.segments
    val totalS = segments.sumOf { it.seconds.toInt() }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(plan.workout.name, style = TvType.section, color = look.ink)
        Text(
            stringResource(R.string.ex_plan_summary, clockText(totalS), segments.size, plan.songs.size),
            style = TvType.body, color = look.ink2,
        )
        MountainProfile(
            profile = plan.profile, marker = null, segments = segments,
            modifier = Modifier.fillMaxWidth().height(PLAN_MOUNTAIN_HEIGHT),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            val ready = state.rebuilding == null
            TvPill(stringResource(R.string.ex_start), { if (ready) session.start(state.built) }, Modifier.tvInitialFocus().tvLeftToDrawer(), icon = Icons.Rounded.PlayArrow, primary = true, height = 38.dp)
            TvPill(stringResource(R.string.ex_another_mix), { if (ready) session.rebuild() }, height = 38.dp)
            TvPill(stringResource(R.string.ex_change), { if (ready) session.backToSetup() }, height = 38.dp)
        }
    }
}

private val PLAN_MOUNTAIN_HEIGHT = 150.dp

private fun clockText(totalS: Int): String = "%d:%02d".format(totalS / 60, totalS % 60)

/**
 * The ride, on this TV: the core's own page in a WebView from the ride server this app runs on 127.0.0.1 (the
 * one renderer the phone-to-TV ride uses), the position following this TV's own player; a few focusable
 * controls underneath (pause, next part, end) and the summary when it is over.
 */
@Composable
fun TvRideOverlay() {
    val app = LocalContext.current.homefi
    val session = app.ride
    val ui by session.ui.collectAsStateWithLifecycle()
    val frame by session.frame.collectAsStateWithLifecycle()
    val tv by session.tv.collectAsStateWithLifecycle()
    var confirmEnd by remember { mutableStateOf(false) }
    Surface(Modifier.fillMaxSize(), color = Color.Black) {
        when (val state = ui) {
            is RideUi.Riding -> {
                BackHandler { confirmEnd = true }
                val local = tv.url?.let(::localUrl)
                Box(Modifier.fillMaxSize()) {
                    if (local != null) {
                        RideWebView(local, followSong = true) { app.player.state.value.positionMs }
                    } else {
                        Text(
                            tv.error ?: stringResource(R.string.ex_building),
                            style = TvType.section,
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.align(Alignment.Center),
                        )
                    }
                    Row(
                        Modifier.align(Alignment.TopCenter).padding(top = 12.dp).background(Color(0x99000000), androidx.compose.foundation.shape.CircleShape).padding(horizontal = 10.dp, vertical = 5.dp),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        val playing = frame?.playing == true
                        TvPill(
                            stringResource(if (playing) R.string.ex_pause else R.string.ex_resume), session::togglePause,
                            Modifier.tvInitialFocus(), icon = if (playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow, primary = true, height = 30.dp,
                        )
                        TvPill(stringResource(R.string.ex_skip), session::skipSegment, icon = Icons.Rounded.SkipNext, height = 30.dp)
                        TvPill(stringResource(R.string.ex_end), { confirmEnd = true }, icon = Icons.Rounded.Stop, height = 30.dp)
                    }
                }
                if (confirmEnd) {
                    TvConfirmDialog(
                        title = stringResource(R.string.ex_end_q),
                        text = stringResource(R.string.ex_paused_d),
                        confirmLabel = stringResource(R.string.ex_end),
                        onConfirm = session::end,
                        onDismiss = { confirmEnd = false },
                    )
                }
            }
            is RideUi.Finished -> {
                BackHandler { session.leave() }
                RideDone(state, session)
            }
            else -> Unit
        }
    }
}

/** The end of a ride, made for the D-pad: the mountain, the numbers, and "Ride again" / "Done". */
@Composable
private fun RideDone(state: RideUi.Finished, session: com.perseusoft.homefi.player.exercise.RideSession) {
    val look = LocalTvLook.current
    val plan = state.plan
    val segments = plan.workout.segments
    Column(Modifier.fillMaxSize().padding(tvSafePadding()).padding(horizontal = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(if (state.completed) R.string.ex_done else R.string.ex_ended), style = TvType.title, color = look.ink)
        MountainProfile(plan.profile, null, Modifier.fillMaxWidth().height(PLAN_MOUNTAIN_HEIGHT), segments = segments)
        Text(
            "${stringResource(R.string.ex_stat_time)}: ${clockText(state.elapsedS)}   ·   ${stringResource(R.string.ex_stat_songs)}: ${plan.songs.size}",
            style = TvType.section, color = look.ink,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            TvPill(stringResource(R.string.ex_again), session::again, Modifier.tvInitialFocus(), icon = Icons.Rounded.PlayArrow, primary = true, height = 38.dp)
            TvPill(stringResource(R.string.tv_train_done), session::leave, height = 38.dp)
        }
        Text(stringResource(R.string.ex_guide), style = TvType.secondary, color = look.ink3)
    }
}

/** The ride server's address as this TV reaches it itself. */
internal fun localUrl(serverUrl: String): String {
    val uri = Uri.parse(serverUrl)
    val port = if (uri.port > 0) ":${uri.port}" else ""
    return "http://127.0.0.1$port${uri.path.orEmpty()}"
}
