package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.lyrics.Lyrics
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.player.PlayerUiState
import com.perseusoft.homefi.ui.player.KaraokeLines
import com.perseusoft.homefi.ui.player.karaokeFrame
import com.perseusoft.homefi.ui.player.rememberLyricsTime
import com.perseusoft.homefi.ui.theme.LyricsPalette

/** Under this height only the line sung and the next one show (a phone held upright). */
private const val ROOMY_DP = 220
private const val DIM_ALPHA = 0.42f
private const val FILL_END_MIX = 0.45f

/**
 * Karaoke in the ride, where the mountain goes: the line being sung fills in the part's [zone] colour, the next one
 * waits under it. The ride plays in the normal player, so the song, its lyrics (the app's cache, nothing fetched twice)
 * and the Bluetooth shift are the player's own; this only draws them.
 */
@Composable
internal fun RideLyrics(zone: Color, modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val player by app.player.state.collectAsStateWithLifecycle()
    val offsetMs by app.settings.settings.collectAsStateWithLifecycle().let { s -> remember { derivedStateOf { s.value.lyricsOffsetMs } } }
    val mediaId = player.current?.mediaId
    LaunchedEffect(mediaId) { mediaId?.let(app.lyrics::request) }
    val states by app.lyrics.states.collectAsStateWithLifecycle()
    val state = mediaId?.let { states[it] ?: LyricsState.Loading }
    val ink = MaterialTheme.colorScheme.onSurface
    val palette = remember(zone, ink) {
        LyricsPalette(ink = ink, dim = ink.copy(alpha = DIM_ALPHA), fill = zone, fillEnd = lerp(zone, Color.White, FILL_END_MIX))
    }
    Box(modifier, contentAlignment = Alignment.Center) {
        val lyrics = (state as? LyricsState.Ready)?.lyrics
        when {
            lyrics != null && lyrics.synced -> SungLines(lyrics, player, offsetMs, palette)
            else -> Note(
                when (state) {
                    null, LyricsState.Loading -> R.string.lyrics_loading
                    is LyricsState.Ready -> R.string.lyrics_not_synced
                    else -> R.string.lyrics_none
                },
            )
        }
    }
}

@Composable
private fun SungLines(lyrics: Lyrics, player: PlayerUiState, offsetMs: Int, palette: LyricsPalette) {
    val time = rememberLyricsTime(player, offsetMs)
    val frame by remember(lyrics) { derivedStateOf { karaokeFrame(lyrics, time.value) } }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val roomy = maxHeight >= ROOMY_DP.dp
        val big = if (roomy) 34.sp else 24.sp
        val small = if (roomy) 18.sp else 14.sp
        val bigStyle = MaterialTheme.typography.headlineMedium.copy(fontSize = big, lineHeight = big * 1.2f, fontWeight = FontWeight.Bold)
        val smallStyle = MaterialTheme.typography.titleMedium.copy(fontSize = small, lineHeight = small * 1.25f, fontWeight = FontWeight.Medium)
        // short: the line already sung goes, so the one sung and the next keep their size
        val shown = if (roomy) frame else frame.copy(previous = -1)
        KaraokeLines(lyrics, shown, time, palette, bigStyle, smallStyle, Modifier.fillMaxSize())
    }
}

@Composable
private fun Note(text: Int) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.padding(horizontal = 24.dp),
    )
}
