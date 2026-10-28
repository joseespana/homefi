package com.perseusoft.homefi.ui.search

import com.perseusoft.homefi.ui.SkeletonList
import androidx.compose.animation.AnimatedContent
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import com.perseusoft.homefi.ui.tidal.tidalSection
import com.perseusoft.homefi.ui.tidal.rememberRowContext
import com.perseusoft.homefi.ui.tidal.TidalViewModel
import com.perseusoft.homefi.ui.tidal.TidalRoutePage
import com.perseusoft.homefi.ui.tidal.CollectionView
import com.perseusoft.homefi.ui.tidal.Collection
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.LaunchedEffect
import com.perseusoft.homefi.ui.SelectChip
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Videocam
import com.perseusoft.homefi.ui.theme.Card
import androidx.compose.material3.CardDefaults
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import coil3.compose.AsyncImage
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.ui.HomefiLogo
import com.perseusoft.homefi.ui.QualityBadge

@Composable
fun SearchScreen(onQueued: () -> Unit, vm: SearchViewModel = viewModel(), tidal: TidalViewModel = viewModel()) {
    val stack by tidal.stack.collectAsStateWithLifecycle()
    // kept here so it survives a visit to an album
    var collection by rememberSaveable { mutableStateOf(Collection.PLAYLISTS) }

    // artist / album / playlist pages stack on top of the search: deeper slides in, back slides out
    val motion = LocalMotion.current
    BackHandler(enabled = stack.isNotEmpty(), onBack = tidal::pop)
    AnimatedContent(
        targetState = stack,
        contentKey = { it.size to it.lastOrNull() },
        transitionSpec = { motion.screens(forward = targetState.size > initialState.size) },
        label = "tidal-stack",
    ) { shown ->
        val route = shown.lastOrNull()
        if (route != null) TidalRoutePage(route, tidal) else SearchHome(vm, tidal, collection, { collection = it }, onQueued)
    }
}

/** What the search area shows under the field. */
private enum class SearchBody { LINK, ERROR, LOADING, COLLECTION, RESULTS }

@Composable
private fun SearchHome(
    vm: SearchViewModel,
    tidal: TidalViewModel,
    collection: Collection,
    onCollection: (Collection) -> Unit,
    onQueued: () -> Unit,
) {
    val state by vm.state.collectAsStateWithLifecycle()
    val focus = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    val motion = LocalMotion.current

    Column(Modifier.fillMaxSize()) {
        OutlinedTextField(
            value = state.query,
            onValueChange = vm::onQueryChange,
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            placeholder = { Text(stringResource(R.string.search_hint)) },
            leadingIcon = { Icon(Icons.Rounded.Search, null) },
            trailingIcon = {
                if (state.query.isNotEmpty()) {
                    IconButton(onClick = { vm.onQueryChange("") }) { Icon(Icons.Rounded.Clear, stringResource(R.string.clear)) }
                }
            },
            singleLine = true,
            shape = RoundedCornerShape(28.dp),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = {
                vm.submit()
                keyboard?.hide() // so the results are visible
                focus.clearFocus()
            }),
        )


        val ctx = rememberRowContext(tidal)
        val body = when {
            state.link != null -> SearchBody.LINK
            state.error != null -> SearchBody.ERROR
            state.loading && state.results == null -> SearchBody.LOADING
            state.results == null -> SearchBody.COLLECTION
            else -> SearchBody.RESULTS
        }
        // the collection, the skeleton and the results fade through one another
        AnimatedContent(targetState = body, transitionSpec = { motion.fadeThrough() }, label = "search-body") { shown ->
        Column(Modifier.fillMaxSize()) {
        val link = state.link
        val results = state.results
        when {
            shown == SearchBody.LINK && link != null -> Card(
                Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
            ) {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.Link, null)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.search_link_detected), fontWeight = FontWeight.SemiBold)
                        Text("${link.kind.path()}/${link.id}", style = MaterialTheme.typography.bodySmall)
                    }
                    FilledTonalButton(onClick = {
                        vm.downloadLink(link)
                        onQueued()
                    }) { Text(stringResource(R.string.download)) }
                }
            }

            shown == SearchBody.ERROR && state.error != null -> Text(
                state.error!!,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(16.dp),
            )

            shown == SearchBody.LOADING -> SkeletonList()

            // before searching: the user's own Tidal collection
            shown == SearchBody.COLLECTION || results == null -> {
                LaunchedEffect(collection) { tidal.loadCollection(collection) }
                Row(
                    Modifier
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Collection.entries.forEach { c ->
                        SelectChip(selected = collection == c, onClick = { onCollection(c) }, label = { Text(stringResource(c.label())) })
                    }
                }
                // the chips' order decides which way the lists slide
                AnimatedContent(
                    targetState = collection,
                    transitionSpec = { motion.screens(forward = targetState.ordinal > initialState.ordinal) },
                    label = "collection",
                ) { CollectionView(it, tidal) }
            }

            // new results cascade in again
            else -> ListEntrance(key = results) {
                LazyColumn(Modifier.fillMaxSize()) {
                    tidalSection(R.string.search_top_hit, listOfNotNull(results.topHit), ctx)
                    tidalSection(R.string.search_tracks, results.tracks.take(10), ctx)
                    tidalSection(R.string.search_artists, results.artists.take(5), ctx)
                    tidalSection(R.string.search_albums, results.albums.take(8), ctx)
                    tidalSection(R.string.search_playlists, results.playlists.take(5), ctx)
                }
            }
        }
        }
        }
    }
}

private fun Collection.label(): Int = when (this) {
    Collection.PLAYLISTS -> R.string.tidal_my_playlists
    Collection.TRACKS -> R.string.tidal_favorite_tracks
    Collection.ALBUMS -> R.string.tidal_favorite_albums
    Collection.ARTISTS -> R.string.tidal_favorite_artists
}

@Composable
private fun EmptySearch() {
    Column(
        Modifier
            .fillMaxSize()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        HomefiLogo(72.dp)
        Spacer(Modifier.size(16.dp))
        Text(
            stringResource(R.string.search_empty),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
