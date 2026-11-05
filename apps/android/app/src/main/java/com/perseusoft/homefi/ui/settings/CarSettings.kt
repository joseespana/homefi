package com.perseusoft.homefi.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

/** Settings → Playback → Android Auto: on / off, the lyric line, and which sections the car shows. */
@Composable
internal fun CarSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.car_title)) {
        SwitchRow(
            stringResource(R.string.car_enabled), settings.carEnabled,
            hint = stringResource(R.string.car_enabled_hint),
        ) { v -> app.settings.update { it.copy(carEnabled = v) } }
        Reveal(settings.carEnabled) {
            Column {
                RowDivider()
                SwitchRow(
                    stringResource(R.string.car_lyrics), settings.carLyrics,
                    hint = stringResource(R.string.car_lyrics_hint),
                ) { v -> app.settings.update { it.copy(carLyrics = v) } }
                RowDivider()
                SwitchRow(stringResource(R.string.car_recent), settings.carRecent) { v -> app.settings.update { it.copy(carRecent = v) } }
                RowDivider()
                SwitchRow(stringResource(R.string.car_playlists), settings.carPlaylists) { v -> app.settings.update { it.copy(carPlaylists = v) } }
                RowDivider()
                SwitchRow(stringResource(R.string.car_albums), settings.carAlbums) { v -> app.settings.update { it.copy(carAlbums = v) } }
                RowDivider()
                SwitchRow(stringResource(R.string.car_artists), settings.carArtists) { v -> app.settings.update { it.copy(carArtists = v) } }
                RowDivider()
                SwitchRow(stringResource(R.string.car_songs), settings.carSongs) { v -> app.settings.update { it.copy(carSongs = v) } }
            }
        }
    }
}
