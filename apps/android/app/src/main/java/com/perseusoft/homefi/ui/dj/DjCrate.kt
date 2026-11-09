package com.perseusoft.homefi.ui.dj

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.library.EmbeddedCover
import com.perseusoft.homefi.player.dj.DJ_DECKS
import com.perseusoft.homefi.player.dj.DjSource
import com.perseusoft.homefi.player.dj.deckLetter
import com.perseusoft.homefi.player.isFilePath
import com.perseusoft.homefi.player.tidalId
import com.perseusoft.homefi.ui.settings.Segmented
import kotlinx.coroutines.delay

private const val SEARCH_DEBOUNCE_MS = 350L
/** Most rows drawn for one query: typing narrows a long library down. */
private const val MAX_ROWS = 200
private val COVER = 44.dp

private enum class CrateTab(val label: Int) {
    LIBRARY(R.string.dj_crate_library),
    QUEUE(R.string.dj_crate_queue),
    TIDAL(R.string.dj_crate_tidal),
}

/** One song the crate offers, whatever its tab. */
private class CrateItem(val key: String, val source: DjSource, val cover: Any?)

/**
 * "Choose a song": the library, the queue and TIDAL in one sheet. [target] is the deck that asked
 * (null from the top bar); a row's A / B buttons load it on either deck, a tap on the row loads
 * the preselected or free deck when the DJ setting says so.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DjCrate(
    target: Int?,
    freeDeck: Int,
    tapLoads: Boolean,
    onLoad: (Int, DjSource) -> Unit,
    onDismiss: () -> Unit,
) {
    val app = LocalContext.current.homefi
    var tab by rememberSaveable { mutableStateOf(CrateTab.LIBRARY) }
    var query by rememberSaveable { mutableStateOf("") }
    val library by app.library.state.collectAsStateWithLifecycle()
    val queue by app.player.state.collectAsStateWithLifecycle()
    val loggedIn = app.account.account.collectAsStateWithLifecycle().value != null
    var tidal by remember { mutableStateOf<List<SearchHit>>(emptyList()) }

    LaunchedEffect(query, tab) {
        if (tab != CrateTab.TIDAL || query.isBlank() || !loggedIn) {
            tidal = emptyList()
            return@LaunchedEffect
        }
        delay(SEARCH_DEBOUNCE_MS)
        tidal = runCatching { app.engine.search(query).tracks }.getOrDefault(emptyList())
    }

    val needle = query.trim()
    val items: List<CrateItem> = when (tab) {
        CrateTab.LIBRARY -> library.tracks.asSequence()
            .filter { needle.isEmpty() || it.matches(needle) }
            .take(MAX_ROWS)
            .map { CrateItem(it.path, DjSource.Local(it.path, it.title, it.artist), EmbeddedCover(it.path)) }
            .toList()
        CrateTab.QUEUE -> queue.queue.mapIndexedNotNull { i, entry ->
            val source = when {
                isFilePath(entry.mediaId) -> DjSource.Local(entry.mediaId, entry.title, entry.artist)
                tidalId(entry.mediaId) != null -> DjSource.Tidal(tidalId(entry.mediaId)!!, entry.title, entry.artist)
                else -> null
            }
            source?.takeIf { needle.isEmpty() || entry.title.contains(needle, true) || entry.artist.contains(needle, true) }
                ?.let { CrateItem("$i:${entry.mediaId}", it, if (isFilePath(entry.mediaId)) EmbeddedCover(entry.mediaId) else entry.artworkUri) }
        }
        CrateTab.TIDAL -> tidal.map { CrateItem(it.id, DjSource.Tidal(it.id, it.title, it.subtitle.substringBefore(" · ")), it.coverUrl) }
    }
    val load = { deck: Int, source: DjSource ->
        onLoad(deck, source)
        onDismiss()
    }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                target?.let { stringResource(R.string.dj_choose_for, deckLetter(it).toString()) } ?: stringResource(R.string.dj_choose),
                style = MaterialTheme.typography.titleLarge,
            )
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                singleLine = true,
                placeholder = { Text(stringResource(R.string.dj_crate_search)) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.fillMaxWidth(),
            )
            Segmented(
                options = CrateTab.entries,
                selected = tab,
                label = { stringResource(it.label) },
                onSelect = { tab = it },
                description = stringResource(R.string.dj_choose),
            )
            val hint = when {
                tab == CrateTab.TIDAL && !loggedIn -> R.string.dj_crate_signin
                tab == CrateTab.TIDAL && needle.isEmpty() -> R.string.dj_crate_tidal_hint
                items.isEmpty() -> R.string.dj_crate_empty
                else -> null
            }
            hint?.let { Text(stringResource(it), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(vertical = 16.dp)) }
            LazyColumn(Modifier.fillMaxWidth().heightIn(max = LIST_MAX_HEIGHT), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 24.dp)) {
                items(items, key = { it.key }) { item ->
                    CrateRow(
                        item,
                        onRow = if (tapLoads) ({ load(target ?: freeDeck, item.source) }) else null,
                        onDeck = { deck -> load(deck, item.source) },
                    )
                }
            }
        }
    }
}

private val LIST_MAX_HEIGHT = 420.dp

private fun LibraryTrack.matches(needle: String) =
    title.contains(needle, true) || artist.contains(needle, true) || album.contains(needle, true)

@Composable
private fun CrateRow(item: CrateItem, onRow: (() -> Unit)?, onDeck: (Int) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (onRow != null) Modifier.clickable(onClick = onRow) else Modifier)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CoverArt(item.cover, COVER)
        Column(Modifier.weight(1f)) {
            Text(item.source.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (item.source.artist.isNotBlank()) {
                Text(item.source.artist, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        repeat(DJ_DECKS) { deck ->
            val description = stringResource(if (deck == 0) R.string.dj_load_a else R.string.dj_load_b)
            FilledTonalButton(
                onClick = { onDeck(deck) },
                modifier = Modifier.semantics { contentDescription = description },
            ) { Text(deckLetter(deck).toString()) }
        }
    }
}
