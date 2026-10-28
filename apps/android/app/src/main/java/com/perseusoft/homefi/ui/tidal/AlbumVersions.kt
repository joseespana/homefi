package com.perseusoft.homefi.ui.tidal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AlbumVersionHit
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.library.CoverArt

/** What the version picker does with a release. */
enum class VersionAction { PLAY, OPEN, DOWNLOAD }

/** "3 versions" on a card's cover (or in a row): opens the picker. */
@Composable
fun VersionsChip(count: Int, onClick: () -> Unit, modifier: Modifier = Modifier, onCover: Boolean = true) {
    val haptics = haptics()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(if (onCover) Color.Black.copy(alpha = 0.62f) else MaterialTheme.colorScheme.secondaryContainer)
            .clickable { haptics.press(); onClick() }
            .padding(horizontal = 8.dp, vertical = 4.dp),
    ) {
        val ink = if (onCover) Color.White else MaterialTheme.colorScheme.onSecondaryContainer
        Icon(Icons.Rounded.Layers, null, tint = ink, modifier = Modifier.padding(end = 4.dp).size(14.dp))
        Text(stringResource(R.string.album_versions_count, count), style = MaterialTheme.typography.labelSmall, color = ink, maxLines = 1)
    }
}

/** A release's labels as badges (the explicit one is the "E" next to the title). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VersionLabels(v: AlbumVersionHit, modifier: Modifier = Modifier, skip: String? = null) {
    val labels = v.labels.filterIndexed { i, label -> v.labelCodes.getOrNull(i) != "explicit" && label != skip }
    if (labels.isEmpty()) return
    FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        labels.forEach { QualityBadge(it) }
    }
}

/** What the badges don't say: size, year, and that Atmos needs an Atmos device. */
@Composable
fun versionMeta(v: AlbumVersionHit): String = listOfNotNull(
    v.tracks.toInt().takeIf { it > 0 }?.let { n ->
        pluralStringResource(R.plurals.album_version_tracks, n, n) +
            (v.extraTracks?.let { " " + stringResource(R.string.album_version_extra, it.toInt()) } ?: "")
    },
    v.year?.toString(),
    if (v.atmos) stringResource(R.string.album_version_atmos) else null,
).joinToString(" · ")

/**
 * Every release of an album: cover, badges and size, and Play / Open / Download each.
 * [current] is the album on screen (its page), marked "Viewing".
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AlbumVersionsSheet(
    title: String,
    versions: List<AlbumVersionHit>,
    current: String?,
    onDismiss: () -> Unit,
    onAction: (AlbumVersionHit, VersionAction) -> Unit,
) {
    val haptics = haptics()
    val act = { v: AlbumVersionHit, a: VersionAction ->
        onDismiss()
        onAction(v, a)
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 16.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    stringResource(R.string.album_versions_count, versions.size),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            LazyColumn(Modifier.padding(top = 8.dp)) {
                items(versions, key = { it.id }) { v ->
                    val here = v.id == current
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(if (here) MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.45f) else Color.Transparent)
                            .clickable(enabled = !here) { haptics.press(); act(v, VersionAction.OPEN) }
                            .semantics { contentDescription = v.description }
                            .padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
                    ) {
                        CoverArt(v.coverUrl, 56.dp, icon = Icons.Rounded.Album, radius = 8.dp)
                        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(
                                    v.title,
                                    style = MaterialTheme.typography.bodyLarge,
                                    fontWeight = FontWeight.SemiBold,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f, fill = false),
                                )
                                if (v.explicit) {
                                    Spacer(Modifier.width(6.dp))
                                    QualityBadge("E")
                                }
                                val tag = when {
                                    here -> R.string.album_version_viewing
                                    v.primary -> R.string.album_version_default
                                    else -> null
                                }
                                if (tag != null) {
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        stringResource(tag).uppercase(),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.primary,
                                        maxLines = 1,
                                    )
                                }
                            }
                            VersionLabels(v, Modifier.padding(vertical = 3.dp))
                            Text(versionMeta(v), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        IconButton(onClick = { haptics.press(); act(v, VersionAction.PLAY) }) {
                            Icon(Icons.Rounded.PlayArrow, stringResource(R.string.album_version_play), tint = MaterialTheme.colorScheme.primary)
                        }
                        IconButton(onClick = { haptics.confirm(); act(v, VersionAction.DOWNLOAD) }) {
                            Icon(Icons.Rounded.Download, stringResource(R.string.album_version_download), tint = MaterialTheme.colorScheme.primary)
                        }
                    }
                }
            }
            Text(
                stringResource(R.string.album_versions_footer),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
}
