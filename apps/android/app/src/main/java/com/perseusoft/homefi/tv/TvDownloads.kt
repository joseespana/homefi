package com.perseusoft.homefi.tv

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.DoNotDisturbOn
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.TaskAlt
import androidx.compose.material.icons.rounded.Usb
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.ActiveTask
import com.perseusoft.homefi.data.DownloadJob
import com.perseusoft.homefi.data.DownloadTarget
import com.perseusoft.homefi.data.LogEntry
import com.perseusoft.homefi.data.LogKind
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.data.TvStorage
import com.perseusoft.homefi.data.VolumeInfo
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.formatBytes
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlinx.coroutines.delay

private const val POLL_BUSY_MS = 3_000L
private const val POLL_IDLE_MS = 10_000L
private val CARD_WIDTH = 214.dp
private val CARD_HEIGHT = 84.dp

/**
 * Downloads (docs/TV_UI.md 6.8): a card per storage volume with its free space (OK picks where downloads go),
 * the target and queue controls, a notice when the drive is low or was pulled out, then the queue with
 * progress. The 500 MB rule itself runs in the core; here it is shown and obeyed.
 */
@Composable
fun TvDownloads(entryKey: Int) {
    val app = LocalContext.current.homefi
    val context = LocalContext.current
    val repo = app.downloads
    val state by repo.state.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val volumes by app.volumes.volumes.collectAsStateWithLifecycle()
    val library by app.library.state.collectAsStateWithLifecycle()
    val tv = settings.tv
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    val internal = volumes.firstOrNull { !it.removable }
    val usb = app.volumes.usb().takeIf { u -> volumes.any { it.root == u?.root } }
    val low = internal != null && TvStorage.isLow(internal.freeBytes, tv.minFreeMb) && tv.downloadTarget == DownloadTarget.INTERNAL
    val sharedAccess = Storage.hasSharedAccess(context)
    var guide by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val unavailableSongs = remember(library.tracks, library.missingRoots) { library.tracks.count { library.unavailable(it.path) } }

    // free space moves while files are written
    LaunchedEffect(state.isBusy) {
        while (true) {
            app.volumes.refresh()
            delay(if (state.isBusy) POLL_BUSY_MS else POLL_IDLE_MS)
        }
    }

    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        TvScreenHeader(
            stringResource(R.string.tv_nav_downloads),
            (if (state.queued > 0) stringResource(R.string.downloads_counter, state.finished, state.queued) + " · " else "") + stringResource(R.string.tv_dl_min, "${tv.minFreeMb} MB"),
            Modifier.padding(start = 12.dp),
        ) { TvNowPlayingChip() }
        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(start = 12.dp, end = 24.dp, top = 12.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            item(key = "volumes") {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(bottom = 4.dp)) {
                    volumes.forEachIndexed { i, v ->
                        val selected = if (v.removable) tv.downloadTarget == DownloadTarget.USB else tv.downloadTarget == DownloadTarget.INTERNAL
                        VolumeCard(
                            v, selected, TvStorage.isLow(v.freeBytes, tv.minFreeMb), sharedAccess,
                            Modifier.then(if (i == 0) Modifier.tvInitialFocus(key = entryKey) else Modifier).then(if (i == 0) Modifier.tvLeftToDrawer() else Modifier),
                        ) { app.settings.updateTv { it.copy(downloadTarget = if (v.removable) DownloadTarget.USB else DownloadTarget.INTERNAL) } }
                    }
                }
            }
            item(key = "controls") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    val targetLabel = stringResource(
                        when (tv.downloadTarget) {
                            DownloadTarget.INTERNAL -> R.string.tv_dl_internal
                            DownloadTarget.USB -> R.string.tv_dl_usb
                            DownloadTarget.ASK -> R.string.tv_dl_ask
                        },
                    )
                    TvPill(
                        stringResource(R.string.tv_dl_target, targetLabel),
                        { app.settings.updateTv { it.copy(downloadTarget = DownloadTarget.entries[(it.downloadTarget.ordinal + 1) % DownloadTarget.entries.size]) } },
                        Modifier.tvLeftToDrawer(),
                        icon = Icons.Rounded.Download,
                        height = 30.dp,
                        labelStyle = TvType.body.copy(fontSize = 14.sp),
                    )
                    if (state.isBusy) TvPill(stringResource(R.string.tv_dl_cancel_all), repo::cancelAll, icon = Icons.Rounded.Close, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                    if (!state.isBusy && state.log.isNotEmpty()) TvPill(stringResource(R.string.tv_dl_clear), repo::clearLog, icon = Icons.Rounded.Delete, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                    if (usb != null && !sharedAccess) TvPill(stringResource(R.string.tv_dl_usb_access), { guide = true }, icon = Icons.Rounded.Usb, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                }
            }
            if (state.paused.isNotEmpty()) item(key = "paused") {
                Notice(
                    stringResource(if (state.pausedForAccess) R.string.tv_dl_access_wait else R.string.tv_dl_usb_gone),
                    stringResource(if (state.pausedForAccess) R.string.tv_dl_access_wait_body else R.string.tv_dl_paused_body, state.paused.size),
                    look.warn,
                ) { TvPill(stringResource(R.string.tv_dl_resume), repo::resume, Modifier.tvLeftToDrawer(), icon = Icons.Rounded.PlayArrow, primary = true, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp)) }
            }
            if (unavailableSongs > 0 && state.paused.isEmpty()) item(key = "gone") {
                Notice(stringResource(R.string.tv_dl_usb_gone), stringResource(R.string.tv_dl_usb_gone_body, unavailableSongs), look.warn) {}
            }
            if (low) item(key = "low") {
                Notice(
                    stringResource(R.string.tv_dl_nospace, internal?.label ?: stringResource(R.string.tv_dl_internal)),
                    stringResource(R.string.tv_dl_min, "${tv.minFreeMb} MB"),
                    look.error,
                ) {
                    if (usb != null) TvPill(stringResource(R.string.tv_dl_use_usb), { app.settings.updateTv { it.copy(downloadTarget = DownloadTarget.USB) } }, Modifier.tvLeftToDrawer(), icon = Icons.Rounded.Usb, primary = true, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                }
            }
            if (!state.isBusy && state.log.isEmpty() && state.paused.isEmpty()) item(key = "empty") {
                Text(stringResource(R.string.downloads_empty), style = TvType.body.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal), color = look.ink2, modifier = Modifier.padding(top = 16.dp, start = 4.dp))
            }
            items(state.active.values.toList(), key = { "task-${it.task}" }) { task -> ActiveRow(task) { menu = task.title to repo::cancelAll } }
            items(state.pending, key = { "job-${it.id}" }) { job -> PendingRow(job) { menu = job.label to { repo.cancelPending(job.id) } } }
            items(state.log, key = { "log-${it.id}" }) { entry -> LogRow(entry) }
        }
    }
    menu?.let { (title, cancel) ->
        TvActionMenu(title, null, listOf(TvAction(Icons.Rounded.Close, stringResource(R.string.cancel), cancel))) { menu = null }
    }
    state.choosing?.let { job -> TargetChoice(job, internalFree = internal, usb = usb, onChoose = repo::choose) }
    if (guide) {
        TvUsbGuide(
            onOpenSettings = {
                guide = false
                openUsbAccessSettings(context)
            },
            onAppFolder = { guide = false },
        )
    }
}

@Composable
private fun VolumeCard(v: VolumeInfo, selected: Boolean, low: Boolean, sharedAccess: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(TvDimens.panelRadius)
    TvFocusable(onClick = onClick, modifier = modifier.width(CARD_WIDTH).height(CARD_HEIGHT), shape = shape, focusedScale = 1.05f) { focused ->
        Column(
            Modifier
                .fillMaxSize()
                .background(if (focused) Color.White else Color.Black.copy(alpha = 0.18f), shape)
                .background(if (focused) Color.Transparent else Color.White.copy(alpha = 0.06f), shape)
                .border(if (selected) 1.5.dp else 1.dp, if (selected) look.accent2 else look.glassLine, shape)
                .padding(10.dp),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            val ink = if (focused) look.onFocus else look.ink
            val sub = if (focused) ROW_SUB_FOCUSED else look.ink2
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(if (v.removable) Icons.Rounded.Usb else Icons.Rounded.PhoneAndroid, null, tint = if (focused) look.onFocus else look.accent2, modifier = Modifier.size(18.dp))
                Text(if (v.removable) v.label else stringResource(R.string.tv_dl_internal_short), style = TvType.cardTitle.copy(fontSize = 14.sp), color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                if (selected) TvTag(stringResource(R.string.tv_dl_selected), tone = look.accent2)
            }
            Column {
                Text(
                    stringResource(R.string.tv_dl_free, formatBytes(v.freeBytes), formatBytes(v.totalBytes)) + if (low) " · " + stringResource(R.string.tv_dl_low) else "",
                    style = TvType.secondary, color = if (low && !focused) look.warn else sub, maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(5.dp))
                Box(Modifier.fillMaxWidth().height(5.dp).background(if (focused) Color(0x22000000) else Color.White.copy(alpha = 0.14f), CircleShape)) {
                    val used by animateFloatAsState(v.usedFraction, LocalMotion.current.tween(400), label = "tv-volume-used")
                    Box(Modifier.fillMaxWidth(used).height(5.dp).background(if (low) look.warn else look.accent2, CircleShape))
                }
                if (v.removable) Text(
                    stringResource(if (sharedAccess) R.string.tv_dl_writes_drive else R.string.tv_dl_writes_app),
                    style = TvType.secondary.copy(fontSize = 11.sp), color = sub, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
    }
}

@Composable
private fun Notice(title: String, body: String, tone: Color, action: @Composable () -> Unit) {
    val look = LocalTvLook.current
    TvGlass(Modifier.fillMaxWidth().border(1.dp, tone.copy(alpha = 0.6f), RoundedCornerShape(TvDimens.panelRadius)), radius = TvDimens.panelRadius) {
        Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(Icons.Rounded.ErrorOutline, null, tint = tone, modifier = Modifier.size(24.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = TvType.body, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(body, style = TvType.secondary, color = look.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            action()
        }
    }
}

@Composable
private fun QueueLine(title: String, detail: String, icon: ImageVector, tint: Color, progress: Float?, indeterminate: Boolean, onLong: (() -> Unit)?, onClick: () -> Unit) {
    val look = LocalTvLook.current
    TvFocusable(onClick = onClick, onLongClick = onLong, modifier = Modifier.fillMaxWidth().height(48.dp).tvLeftToDrawer(), shape = RoundedCornerShape(12.dp), focusedScale = 1.02f, glow = false) { focused ->
        Row(
            Modifier.fillMaxSize().then(if (focused) Modifier.background(Color.White, RoundedCornerShape(12.dp)) else Modifier).padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(icon, null, tint = if (focused) look.onFocus else tint, modifier = Modifier.size(22.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = TvType.cardTitle.copy(fontSize = 14.sp), color = if (focused) look.onFocus else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(detail, style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (progress != null) TvProgress(progress, Modifier.padding(top = 3.dp), height = 3.dp)
                else if (indeterminate) TvProgress(0.15f, Modifier.padding(top = 3.dp), height = 3.dp)
            }
        }
    }
}

@Composable
private fun ActiveRow(task: ActiveTask, onLong: () -> Unit) {
    val total = task.total
    val detail = "${task.quality} · ${formatBytes(task.downloaded)}" + (total?.let { " / ${formatBytes(it)}" } ?: "")
    QueueLine(task.title, detail, Icons.Rounded.Download, LocalTvLook.current.accent2, total?.takeIf { it > 0 }?.let { task.downloaded.toFloat() / it }, total == null || total == 0L, onLong) { onLong() }
}

@Composable
private fun PendingRow(job: DownloadJob, onLong: () -> Unit) {
    QueueLine(job.label, stringResource(R.string.tv_dl_waiting), Icons.Rounded.Download, LocalTvLook.current.ink3, null, false, onLong) { onLong() }
}

@Composable
private fun LogRow(entry: LogEntry) {
    val look = LocalTvLook.current
    val app = LocalContext.current.homefi
    val (icon, tint) = when (entry.kind) {
        LogKind.DONE -> Icons.Rounded.CheckCircle to look.ok
        LogKind.EXISTS -> Icons.Rounded.TaskAlt to look.accent2
        LogKind.SKIPPED -> Icons.Rounded.DoNotDisturbOn to look.ink3
        LogKind.ERROR -> Icons.Rounded.ErrorOutline to look.error
    }
    val path = entry.path
    val detail = if (entry.kind == LogKind.ERROR) stringResource(R.string.tv_dl_failed) + " · " + entry.detail else entry.detail
    QueueLine(entry.title, detail, icon, tint, null, false, null) { if (path != null) app.player.playFile(path) }
}

/** "Internal or USB?" for `tv.downloadTarget = ask`. */
@Composable
private fun TargetChoice(job: DownloadJob, internalFree: VolumeInfo?, usb: VolumeInfo?, onChoose: (DownloadTarget?) -> Unit) {
    val look = LocalTvLook.current
    Dialog(onDismissRequest = { onChoose(null) }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(360.dp), solid = true) {
                Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(job.label, style = TvType.section, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Text(stringResource(R.string.tv_dl_choose), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2)
                    TvPill(stringResource(R.string.tv_dl_internal) + internalFree?.let { " · " + formatBytes(it.freeBytes) }.orEmpty(), { onChoose(DownloadTarget.INTERNAL) }, Modifier.tvInitialFocus().fillMaxWidth(), icon = Icons.Rounded.PhoneAndroid, primary = true)
                    TvPill(stringResource(R.string.tv_dl_usb) + usb?.let { " · " + formatBytes(it.freeBytes) }.orEmpty(), { onChoose(DownloadTarget.USB) }, Modifier.fillMaxWidth(), icon = Icons.Rounded.Usb)
                    TvPill(stringResource(R.string.cancel), { onChoose(null) }, Modifier.fillMaxWidth())
                }
            }
        }
    }
}
