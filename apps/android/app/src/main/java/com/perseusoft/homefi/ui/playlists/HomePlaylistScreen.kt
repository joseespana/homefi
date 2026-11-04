package com.perseusoft.homefi.ui.playlists

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.HomefiApplication
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ConcertMode
import com.perseusoft.homefi.core.ConcertProgress
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.core.PlaylistOrigin
import com.perseusoft.homefi.core.SetlistSource
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.library.ScreenHeader
import kotlinx.coroutines.launch

/** A playlist HomeFi saved: the same preview as the draft, with Search again and Delete. */
@Composable
fun HomePlaylistScreen(id: String, onBack: () -> Unit, onDelete: (HomePlaylist) -> Unit, onChangeAnswers: (HomePlaylist) -> Unit) {
    val app = LocalContext.current.homefi
    val playlists by app.playlists.list.collectAsStateWithLifecycle()
    val playlist = playlists.firstOrNull { it.id == id } ?: return
    val vm: ConcertViewModel = viewModel(key = "playlist-$id")
    val ui by vm.ui.collectAsStateWithLifecycle()
    var showsOpen by rememberSaveable { mutableStateOf(false) }
    val origin = playlist.concert
    val aiName = (origin?.source as? SetlistSource.Ai)?.let { app.ai.provider(it.providerId)?.name }
    // a party playlist is rebuilt from its answers, a chat one by chatting on; both on their own screen
    val rebuiltOnItsScreen = playlist.origin is PlaylistOrigin.Party || playlist.origin is PlaylistOrigin.Chat
    val chat = playlist.origin is PlaylistOrigin.Chat

    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item(key = "top") { ScreenHeader(playlist.name, onBack = onBack) }
        (ui as? ConcertUi.Building)?.let { building ->
            item(key = "busy") { BuildProgress(building, vm::cancel) }
        }
        (ui as? ConcertUi.Failed)?.let { failed ->
            item(key = "failed") {
                Text(failed.message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = 16.dp))
            }
        }
        playlistPreview(
            playlist = playlist,
            fallbackNote = null,
            aiName = aiName,
            actions = PreviewActions(
                saved = true,
                onSearchAgain = if (rebuiltOnItsScreen) {
                    { onChangeAnswers(playlist) }
                } else {
                    {
                        // the shows are searched again with the AI; Tidal's most played just rebuilds
                        vm.build(origin?.artist.orEmpty(), origin?.showsWanted?.toInt() ?: app.settings.playlists.value.concertShows.toInt(), origin?.source is SetlistSource.TidalPopular, replaceId = playlist.id)
                    }
                },
                onDelete = { onDelete(playlist) },
                onMode = { mode: ConcertMode -> rearrange(vm, playlist, mode, saved = true) },
                searchAgainLabel = when {
                    chat -> R.string.pl_chat_continue
                    rebuiltOnItsScreen -> R.string.pl_party_change_answers
                    else -> R.string.pl_search_again
                },
            ),
            showsOpen = showsOpen,
            onToggleShows = { showsOpen = !showsOpen },
        )
    }
}

/**
 * Removes [playlist] from HomeFi (its own file only) and offers "Undo", which saves it again.
 * Runs in the app scope: the screen that asked has usually left by the time the Snackbar shows.
 */
fun deleteWithUndo(app: HomefiApplication, host: SnackbarHostState, playlist: HomePlaylist) {
    app.appScope.launch {
        val failure = app.playlists.delete(playlist.id)
        val message = failure ?: app.getString(R.string.pl_deleted)
        host.currentSnackbarData?.dismiss()
        val result = host.showSnackbar(
            message,
            actionLabel = if (failure == null) app.getString(R.string.pl_undo) else null,
            duration = SnackbarDuration.Long,
        )
        if (result == SnackbarResult.ActionPerformed) app.playlists.save(playlist)
    }
}
