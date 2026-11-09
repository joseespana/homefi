package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SystemUpdate
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ReleaseNotes
import com.perseusoft.homefi.data.UpdateStatus
import com.perseusoft.homefi.data.UpdateUi
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.releaseNotesText
import kotlinx.coroutines.launch

private const val SCROLL_STEP_PX = 240f

/** The update card shows for a found update that was not sent away with "Later". */
internal fun updateCardVisible(ui: UpdateUi): Boolean =
    ui.release != null && !ui.dismissed &&
        ui.status in setOf(UpdateStatus.AVAILABLE, UpdateStatus.DOWNLOADING, UpdateStatus.READY, UpdateStatus.ERROR)

/**
 * "HomeFi X is available": a glass card over any screen with the download's progress and the
 * D-pad actions: the main one (update / install / retry) takes focus when it can be pressed,
 * "What's new" and "Later" are beside it.
 */
@Composable
fun TvUpdateCard(ui: UpdateUi, onWhatsNew: () -> Unit, modifier: Modifier = Modifier) {
    val updates = LocalContext.current.homefi.updates
    val look = LocalTvLook.current
    val release = ui.release ?: return
    val title = when (ui.status) {
        UpdateStatus.DOWNLOADING -> stringResource(R.string.update_downloading, release.version)
        UpdateStatus.READY -> stringResource(R.string.update_ready, release.version)
        UpdateStatus.ERROR -> stringResource(R.string.update_failed, release.version)
        else -> stringResource(if (release.prerelease) R.string.update_available_beta else R.string.update_available, release.version)
    }
    val hint = when (ui.status) {
        UpdateStatus.ERROR -> ui.error.orEmpty()
        UpdateStatus.READY -> stringResource(R.string.tv_update_ready_hint)
        UpdateStatus.DOWNLOADING -> stringResource(R.string.tv_update_downloading_hint)
        else -> stringResource(R.string.tv_update_available_hint)
    }
    TvGlass(modifier.width(560.dp), solid = true) {
        Column(Modifier.padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(34.dp).background(look.accent.copy(alpha = 0.2f), CircleShape), contentAlignment = Alignment.Center) {
                    Icon(Icons.Rounded.SystemUpdate, null, tint = look.accent2, modifier = Modifier.size(20.dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(title, style = TvType.section, color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(hint, style = TvType.secondary.copy(fontSize = 13.sp), color = if (ui.status == UpdateStatus.ERROR) look.error else look.ink2, maxLines = 3)
                }
            }
            if (ui.status == UpdateStatus.DOWNLOADING) {
                TvProgress(if (ui.total > 0) ui.downloaded.toFloat() / ui.total else 0f, height = 5.dp)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                when (ui.status) {
                    UpdateStatus.READY -> TvPill(
                        stringResource(R.string.tv_update_install_now),
                        updates::install,
                        Modifier.tvInitialFocus(key = ui.status),
                        primary = true,
                        height = 34.dp,
                    )
                    UpdateStatus.AVAILABLE, UpdateStatus.ERROR -> TvPill(
                        stringResource(if (ui.status == UpdateStatus.ERROR) R.string.update_retry else R.string.tv_update_now),
                        { updates.download(installWhenReady = false) },
                        Modifier.tvInitialFocus(key = ui.status),
                        primary = true,
                        height = 34.dp,
                    )
                    else -> Unit
                }
                TvPill(stringResource(R.string.update_whats_new), onWhatsNew, height = 34.dp)
                TvPill(stringResource(R.string.update_later), updates::dismiss, height = 34.dp)
            }
        }
    }
}

/**
 * The one-time "Install unknown apps" step on a TV, in big words: what to press, the path through
 * Settings in case the TV has no direct page, and the single button that opens it.
 */
@Composable
fun TvInstallPermission(onLater: () -> Unit) {
    val updates = LocalContext.current.homefi.updates
    val look = LocalTvLook.current
    var notFound by remember { mutableStateOf(false) }
    BackHandler(onBack = onLater)
    Box(Modifier.fillMaxSize().background(Color(0xE6040510)), contentAlignment = Alignment.Center) {
        TvGlass(Modifier.width(620.dp), solid = true) {
            Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.tv_perm_title), style = TvType.title.copy(fontSize = 28.sp), color = look.ink)
                Text(stringResource(R.string.tv_perm_lead), style = TvType.body.copy(fontWeight = FontWeight.Normal), color = look.ink2)
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(R.string.tv_perm_step1, R.string.tv_perm_step2, R.string.tv_perm_step3).forEachIndexed { i, step ->
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            Box(Modifier.size(28.dp).background(look.gradient, CircleShape), contentAlignment = Alignment.Center) {
                                Text("${i + 1}", style = TvType.body.copy(fontWeight = FontWeight.ExtraBold), color = look.onAccent)
                            }
                            Text(stringResource(step), style = TvType.section, color = look.ink)
                        }
                    }
                }
                Text(stringResource(R.string.tv_perm_path), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink3)
                if (notFound) Text(stringResource(R.string.tv_perm_not_found), style = TvType.secondary.copy(fontSize = 13.sp), color = look.warn)
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(top = 6.dp)) {
                    TvPill(
                        stringResource(R.string.tv_perm_open),
                        { notFound = !updates.openInstallPermission() },
                        Modifier.tvInitialFocus(),
                        primary = true,
                        height = 40.dp,
                    )
                    TvPill(stringResource(R.string.update_later), onLater, height = 40.dp)
                }
            }
        }
    }
}

/** "What's new" as text for the TV: the version, its summary and each section's bullets. */
internal fun releaseNotesAnnotated(notes: List<ReleaseNotes>): AnnotatedString = buildAnnotatedString {
    notes.forEachIndexed { index, release ->
        if (index > 0) append("\n\n")
        withStyle(SpanStyle(fontWeight = FontWeight.ExtraBold)) { append("HomeFi ${release.version}") }
        release.summary?.let { append("\n" + it.spans.joinToString("") { s -> s.text }) }
        release.sections.forEach { section ->
            append("\n\n")
            withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(section.title) }
            section.items.forEach { item -> append("\n•  " + item.spans.joinToString("") { s -> s.text }) }
        }
    }
}

/** Scrollable text in a glass card with one focused Close pill; up and down scroll the text. */
@Composable
fun TvTextDialog(title: String, body: AnnotatedString, onClose: () -> Unit) {
    val look = LocalTvLook.current
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    BackHandler(onBack = onClose)
    Box(Modifier.fillMaxSize().background(Color(0xE6040510)), contentAlignment = Alignment.Center) {
        TvGlass(Modifier.width(640.dp), solid = true) {
            Column(Modifier.padding(26.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(title, style = TvType.title.copy(fontSize = 26.sp), color = look.ink, maxLines = 2)
                Text(
                    body,
                    style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 16.sp, lineHeight = 23.sp),
                    color = look.ink,
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 230.dp)
                        .onPreviewKeyEvent { e ->
                            // up / down read the text; at its end the focus moves on to Close
                            val down = e.key == Key.DirectionDown
                            val up = e.key == Key.DirectionUp
                            if ((!down && !up) || e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                            val atEnd = if (down) scroll.value >= scroll.maxValue else scroll.value <= 0
                            if (atEnd) return@onPreviewKeyEvent false
                            scope.launch { scroll.animateScrollBy(if (down) SCROLL_STEP_PX else -SCROLL_STEP_PX) }
                            true
                        }
                        .focusable()
                        .verticalScroll(scroll),
                )
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    TvPill(stringResource(R.string.update_close), onClose, Modifier.tvInitialFocus(), primary = true, height = 38.dp)
                    if (scroll.maxValue > 0) Text(stringResource(R.string.tv_notes_scroll), style = TvType.secondary, color = look.ink3)
                }
            }
        }
    }
}

/** Markdown release notes of the update that was found (what the phone's dialog shows). */
internal fun updateNotesAnnotated(markdown: String, fallback: String): AnnotatedString =
    markdown.takeIf { it.isNotBlank() }?.let(::releaseNotesText) ?: AnnotatedString(fallback)
