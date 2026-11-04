package com.perseusoft.homefi.ui.playlists

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.SetlistSource
import com.perseusoft.homefi.core.SourceLink

private const val LINK_MIN_HEIGHT_DP = 56

/**
 * Where a result came from: who searched, the pages they read (when [showLinks]) and, always
 * when Google gave one, its Search Suggestions widget.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SourcesSheet(playlist: HomePlaylist, aiName: String?, showLinks: Boolean, onDismiss: () -> Unit) {
    val context = LocalContext.current
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 12.dp)) {
            Text(
                stringResource(R.string.pl_sources),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            val ai = (playlist.concert?.source as? SetlistSource.Ai)
            if (ai != null) {
                Text(
                    stringResource(R.string.pl_sources_ai, aiName ?: ai.model),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                )
            }
            if (showLinks) {
                playlist.sources.forEach { link -> SourceRow(link) { openLink(context, link.url) } }
            }
            playlist.searchWidgetHtml?.let { WidgetSection(it) }
        }
    }
}

/** Google's heading and the widget, unmodified: shown whenever the result carries it. */
@Composable
fun WidgetSection(html: String, modifier: Modifier = Modifier) {
    Column(modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(
            stringResource(R.string.pl_google_suggestions),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(bottom = 4.dp),
        )
        SearchSuggestions(html, unavailableText = stringResource(R.string.pl_widget_unavailable), modifier = Modifier.fillMaxWidth())
    }
}

@Composable
private fun SourceRow(link: SourceLink, onOpen: () -> Unit) {
    ListItem(
        modifier = Modifier.heightIn(min = LINK_MIN_HEIGHT_DP.dp).clickable(onClick = onOpen),
        headlineContent = { Text(link.title.ifBlank { link.url }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        supportingContent = { Text(link.url, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        trailingContent = { Icon(Icons.AutoMirrored.Rounded.OpenInNew, null) },
    )
}

private fun openLink(context: android.content.Context, url: String) {
    val uri = Uri.parse(url)
    if (uri.scheme != "https" && uri.scheme != "http") return
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        // no browser: nothing to open
    }
}
