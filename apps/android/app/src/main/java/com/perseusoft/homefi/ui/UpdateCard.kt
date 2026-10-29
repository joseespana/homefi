package com.perseusoft.homefi.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.SystemUpdate
import androidx.compose.material3.AlertDialog
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AppRelease
import com.perseusoft.homefi.data.UpdateStatus
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.theme.LocalMotion

/** Release notes (Markdown) as plain text: headings in bold, bullets as "•", no markup. */
fun releaseNotesText(markdown: String): AnnotatedString = buildAnnotatedString {
    val clean = { s: String -> s.replace(Regex("\\[([^]]+)]\\([^)]*\\)"), "$1").replace("**", "").replace("`", "") }
    var first = true
    for (raw in markdown.lines()) {
        val line = raw.trim()
        if (line.isEmpty() || line.matches(Regex("-{3,}"))) continue
        if (!first) append('\n')
        first = false
        val heading = Regex("^#{1,6}\\s+(.*)$").find(line)
        val bullet = Regex("^[-*+]\\s+(.*)$").find(line)
        when {
            heading != null -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(clean(heading.groupValues[1])) }
            bullet != null -> append("•  ").also { append(clean(bullet.groupValues[1])) }
            else -> append(clean(line))
        }
    }
}

/** "What's new in HomeFi X": the release notes, and the release page. */
@Composable
fun WhatsNewDialog(release: AppRelease, onDismiss: () -> Unit, onOpenPage: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.update_whats_new_title, release.version)) },
        text = {
            Text(
                release.notes.takeIf { it.isNotBlank() }?.let(::releaseNotesText) ?: AnnotatedString(stringResource(R.string.update_no_notes)),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.heightIn(max = 360.dp).verticalScroll(rememberScrollState()),
            )
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.update_close)) } },
        dismissButton = { TextButton(onClick = onOpenPage) { Text(stringResource(R.string.update_release_page)) } },
    )
}

/** Android asks to allow installing apps from HomeFi once: what to do there. */
@Composable
fun InstallPermissionDialog(onOpen: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.update_permission_title)) },
        text = { Text(stringResource(R.string.update_permission_text)) },
        confirmButton = { Button(onClick = onOpen) { Text(stringResource(R.string.update_permission_open)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.update_later)) } },
    )
}

/**
 * "HomeFi X is available · What's new · Update": a card at the top, over any screen, with
 * the download's progress. Hidden with "Later" until the app is opened again.
 */
@Composable
fun UpdateCard(modifier: Modifier = Modifier) {
    val updates = LocalContext.current.homefi.updates
    if (!updates.enabled) return
    val ui by updates.state.collectAsStateWithLifecycle()
    val release = ui.release
    val visible = release != null && !ui.dismissed &&
        ui.status in setOf(UpdateStatus.AVAILABLE, UpdateStatus.DOWNLOADING, UpdateStatus.READY, UpdateStatus.ERROR)
    var notes by remember { mutableStateOf(false) }
    var shown by remember { mutableStateOf(release) }
    if (release != null) shown = release

    if (ui.needsPermission) {
        InstallPermissionDialog(onOpen = updates::openInstallPermission, onDismiss = updates::skipPermission)
    }

    val motion = LocalMotion.current
    AnimatedVisibility(
        visible,
        modifier.statusBarsPadding(),
        enter = if (motion.moves) slideInVertically(motion.move()) { -it } + fadeIn(motion.tween(200)) else fadeIn(motion.tween(200)),
        exit = if (motion.moves) slideOutVertically(motion.move()) { -it } + fadeOut(motion.tween(150)) else fadeOut(motion.tween(150)),
    ) {
        val r = shown ?: return@AnimatedVisibility
        if (notes) WhatsNewDialog(r, onDismiss = { notes = false }, onOpenPage = updates::openPage)
        Card(
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
            elevation = CardDefaults.cardElevation(defaultElevation = 6.dp),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
        ) {
            Column(Modifier.padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.SystemUpdate, null, tint = MaterialTheme.colorScheme.onPrimaryContainer)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(
                            when (ui.status) {
                                UpdateStatus.DOWNLOADING -> stringResource(R.string.update_downloading, r.version)
                                UpdateStatus.READY -> stringResource(R.string.update_ready, r.version)
                                UpdateStatus.ERROR -> stringResource(R.string.update_failed, r.version)
                                else -> stringResource(if (r.prerelease) R.string.update_available_beta else R.string.update_available, r.version)
                            },
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            when (ui.status) {
                                UpdateStatus.ERROR -> ui.error ?: ""
                                UpdateStatus.READY -> stringResource(R.string.update_install_hint)
                                else -> stringResource(R.string.update_music_hint)
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                            maxLines = 3,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    IconButton(onClick = updates::dismiss) {
                        Icon(Icons.Rounded.Close, stringResource(R.string.update_later), tint = MaterialTheme.colorScheme.onPrimaryContainer)
                    }
                }
                if (ui.status == UpdateStatus.DOWNLOADING) {
                    val fraction = if (ui.total > 0) (ui.downloaded.toFloat() / ui.total).coerceIn(0f, 1f) else null
                    if (fraction != null) {
                        LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth().padding(top = 8.dp, end = 12.dp))
                    } else {
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth().padding(top = 8.dp, end = 12.dp))
                    }
                }
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { notes = true }) { Text(stringResource(R.string.update_whats_new)) }
                    Spacer(Modifier.weight(1f))
                    when (ui.status) {
                        UpdateStatus.AVAILABLE, UpdateStatus.ERROR ->
                            Button(onClick = updates::download, modifier = Modifier.padding(end = 8.dp)) {
                                Text(stringResource(if (ui.status == UpdateStatus.ERROR) R.string.update_retry else R.string.update_update))
                            }
                        UpdateStatus.READY ->
                            Button(onClick = updates::install, modifier = Modifier.padding(end = 8.dp)) { Text(stringResource(R.string.update_install)) }
                        else -> Unit
                    }
                }
            }
        }
    }
}
