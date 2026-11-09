package com.perseusoft.homefi.tv

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.OpenInNew
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.ItemKind
import com.perseusoft.homefi.core.LibraryTrack
import com.perseusoft.homefi.core.SearchHit
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.playlists.songSeedAction
import com.perseusoft.homefi.ui.playlists.toSeed
import com.perseusoft.homefi.ui.tidal.TidalViewModel

// What a long press of OK offers: the phone's row actions (play next, add to queue, Sound, Playlist
// like this), plus Play and (TIDAL) Download. One list builder per kind of row.

/** The actions of a song of the library. */
@Composable
fun libraryTrackActions(track: LibraryTrack): List<TvAction> {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val seed = songSeedAction { track.toSeed() }
    return buildList {
        add(TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.play(listOf(track)); nav.openPlayer() })
        add(TvAction(Icons.Rounded.SkipNext, stringResource(R.string.queue_play_next)) { app.player.queueTracks(listOf(track), next = true) })
        add(TvAction(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add)) { app.player.queueTracks(listOf(track), next = false) })
        add(TvAction(Icons.Rounded.Tune, stringResource(R.string.sound_menu_item)) {
            app.sound.openMenu(TasteScope.Track(id = track.path, name = track.title))
            nav.go(TvSection.SOUND)
        })
        if (seed != null) add(TvAction(AiIcon, stringResource(R.string.pl_like_this)) { seed(); nav.go(TvSection.PLAYLISTS) })
    }
}

/** The actions of a TIDAL row: a song plays, queues and downloads; an album, artist or playlist opens. */
@Composable
fun tidalHitActions(hit: SearchHit, vm: TidalViewModel): List<TvAction> {
    val app = LocalContext.current.homefi
    val nav = LocalTvNav.current
    val seed = songSeedAction { hit.toSeed() }
    return buildList {
        if (hit.kind == ItemKind.TRACK) {
            add(TvAction(Icons.Rounded.PlayArrow, stringResource(R.string.player_play)) { app.player.playTidal(listOf(hit)); nav.openPlayer() })
            add(TvAction(Icons.Rounded.SkipNext, stringResource(R.string.queue_play_next)) { app.player.queueTidal(listOf(hit), next = true) })
            add(TvAction(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add)) { app.player.queueTidal(listOf(hit), next = false) })
        } else {
            add(TvAction(Icons.Rounded.OpenInNew, stringResource(R.string.tv_act_open)) { vm.open(hit) })
        }
        if (hit.kind in DOWNLOADABLE) add(TvAction(Icons.Rounded.Download, stringResource(R.string.download)) { vm.download(hit) })
        if (hit.kind == ItemKind.TRACK) {
            add(TvAction(Icons.Rounded.Tune, stringResource(R.string.sound_menu_item)) {
                app.sound.openMenu(TasteScope.Track(id = hit.id, name = hit.title))
                nav.go(TvSection.SOUND)
            })
            if (seed != null) add(TvAction(AiIcon, stringResource(R.string.pl_like_this)) { seed(); nav.go(TvSection.PLAYLISTS) })
        }
    }
}

private val DOWNLOADABLE = setOf(ItemKind.TRACK, ItemKind.ALBUM, ItemKind.PLAYLIST)

/** Play next / add to queue of a whole list (an album page's Queue pill). */
@Composable
fun queueActions(onQueue: (next: Boolean) -> Unit): List<TvAction> = listOf(
    TvAction(Icons.Rounded.SkipNext, stringResource(R.string.queue_play_next)) { onQueue(true) },
    TvAction(Icons.AutoMirrored.Rounded.QueueMusic, stringResource(R.string.queue_add)) { onQueue(false) },
)
