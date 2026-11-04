package com.perseusoft.homefi.ui.playlists

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ConcertMode
import com.perseusoft.homefi.core.ConcertProgress
import com.perseusoft.homefi.core.HomePlaylist
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.library.ScreenHeader
import com.perseusoft.homefi.ui.settings.AiSettingsSection
import com.perseusoft.homefi.ui.settings.MAX_CONCERT_SHOWS
import com.perseusoft.homefi.ui.settings.MIN_CONCERT_SHOWS
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.FilledTonalButton

/**
 * "Get ready for a concert": the artist, how many shows, where the songs will come from, and
 * the result with Play / Save. [onDelete] removes a saved result (the caller offers Undo).
 */
@Composable
fun ConcertScreen(onBack: () -> Unit, onDelete: (HomePlaylist) -> Unit, vm: ConcertViewModel = viewModel()) {
    val app = LocalContext.current.homefi
    val ui by vm.ui.collectAsStateWithLifecycle()
    val settings by app.settings.playlists.collectAsStateWithLifecycle()
    var artist by rememberSaveable { mutableStateOf("") }
    var shows by rememberSaveable(settings.concertShows) { mutableIntStateOf(settings.concertShows.toInt()) }
    var showsOpen by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) { vm.refreshAi() }

    val build: (Boolean) -> Unit = { popular -> if (artist.isNotBlank()) vm.build(artist, shows, popular) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item(key = "top") { ScreenHeader(stringResource(R.string.pl_concert_title), onBack = onBack) }
        when (val state = ui) {
            is ConcertUi.Built -> {
                val playlist = state.built.playlist
                playlistPreview(
                    playlist = playlist,
                    fallbackNote = state.built.fallbackNote,
                    aiName = null,
                    actions = PreviewActions(
                        saved = state.saved,
                        onSave = vm::save,
                        onDiscard = vm::discard,
                        onSearchAgain = { vm.build(playlist.concert?.artist ?: artist, shows, false, replaceId = playlist.id) },
                        onDelete = { onDelete(playlist); vm.discard() },
                        onMode = { mode -> rearrange(vm, playlist, mode, state.saved) },
                    ),
                    showsOpen = showsOpen,
                    onToggleShows = { showsOpen = !showsOpen },
                )
            }
            else -> form(
                ui = state,
                vm = vm,
                artist = artist,
                onArtist = { artist = it },
                shows = shows,
                onShows = { shows = it.coerceIn(MIN_CONCERT_SHOWS, MAX_CONCERT_SHOWS) },
                onBuild = build,
                tidalConnected = app.account.isLoggedIn,
            )
        }
    }
}

/** Re-aggregates the shows already in [playlist]: one show or the whole tour. */
internal fun rearrange(vm: ConcertViewModel, playlist: HomePlaylist, mode: ConcertMode, saved: Boolean) {
    val origin = playlist.concert ?: return
    vm.rearrange(playlist, mode, origin.showsWanted.toInt(), origin.thresholds, saved)
}

private fun LazyListScope.form(
    ui: ConcertUi,
    vm: ConcertViewModel,
    artist: String,
    onArtist: (String) -> Unit,
    shows: Int,
    onShows: (Int) -> Unit,
    onBuild: (popularOnly: Boolean) -> Unit,
    tidalConnected: Boolean,
) {
    val building = ui as? ConcertUi.Building
    item(key = "form") {
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.pl_concert_intro), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            OutlinedTextField(
                value = artist,
                onValueChange = onArtist,
                modifier = Modifier.fillMaxWidth(),
                enabled = building == null,
                label = { Text(stringResource(R.string.pl_artist)) },
                placeholder = { Text(stringResource(R.string.pl_artist_hint)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { onBuild(false) }),
            )
            ShowsStepper(shows, enabled = building == null, onShows)
            SourceNotice(vm, tidalConnected)
            if (ui is ConcertUi.Failed) {
                Text(ui.message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            }
            if (building != null) {
                BuildProgress(building, vm::cancel)
            } else {
                val haptics = haptics()
                Button(
                    onClick = { haptics.press(); onBuild(false) },
                    enabled = artist.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.pl_build)) }
                FilledTonalButton(
                    onClick = { haptics.press(); onBuild(true) },
                    enabled = artist.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.pl_use_popular)) }
            }
        }
    }
}

@Composable
private fun ShowsStepper(value: Int, enabled: Boolean, onChange: (Int) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.pl_shows), style = MaterialTheme.typography.bodyLarge)
            Text(stringResource(R.string.pl_shows_value, value), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = { onChange(value - 1) }, enabled = enabled && value > MIN_CONCERT_SHOWS) {
            Icon(Icons.Rounded.Remove, stringResource(R.string.pl_shows_fewer))
        }
        IconButton(onClick = { onChange(value + 1) }, enabled = enabled && value < MAX_CONCERT_SHOWS) {
            Icon(Icons.Rounded.Add, stringResource(R.string.pl_shows_more))
        }
    }
}

/** Where the songs will come from, said before building. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SourceNotice(vm: ConcertViewModel, tidalConnected: Boolean) {
    val ai by vm.ai.collectAsStateWithLifecycle()
    var setup by rememberSaveable { mutableStateOf(false) }
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        when (val stand = ai) {
            AiStand.None -> {
                Text(stringResource(R.string.pl_no_ai), style = MaterialTheme.typography.bodyMedium, color = muted)
                OutlinedButton(onClick = { setup = true }) { Text(stringResource(R.string.pl_setup_ai)) }
            }
            is AiStand.NoWebSearch ->
                Text(stringResource(R.string.pl_no_web_search, stand.name), style = MaterialTheme.typography.bodyMedium, color = muted)
            is AiStand.Searching ->
                Text(stringResource(R.string.pl_uses_ai, stand.name, stand.searches), style = MaterialTheme.typography.bodyMedium, color = muted)
        }
        if (!tidalConnected) {
            Text(stringResource(R.string.pl_no_tidal), style = MaterialTheme.typography.bodyMedium, color = muted)
        }
    }
    if (setup) {
        ModalBottomSheet(
            onDismissRequest = { setup = false; vm.refreshAi() },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).navigationBarsPadding()) {
                AiSettingsSection()
            }
        }
    }
}

@Composable
internal fun BuildProgress(building: ConcertUi.Building, onCancel: () -> Unit) {
    val progress = building.progress
    val line = when {
        progress is ConcertProgress.ShowsFound -> stringResource(R.string.pl_progress_shows, progress.count.toInt())
        progress is ConcertProgress.Resolving -> stringResource(R.string.pl_progress_resolving, progress.done.toInt(), progress.total.toInt())
        progress is ConcertProgress.Searching && !progress.withAi -> stringResource(R.string.pl_progress_popular)
        building.aiName != null -> stringResource(R.string.pl_progress_ai, building.aiName)
        else -> stringResource(R.string.pl_progress_popular)
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(line, style = MaterialTheme.typography.bodyMedium)
        if (progress is ConcertProgress.Resolving && progress.total > 0u) {
            LinearProgressIndicator(progress = { progress.done.toFloat() / progress.total.toFloat() }, modifier = Modifier.fillMaxWidth())
        } else {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        TextButton(onClick = onCancel) { Text(stringResource(R.string.pl_cancel)) }
    }
}
