package com.perseusoft.homefi.ui.player

import android.graphics.Rect
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.library.CoverArt
import com.perseusoft.homefi.lyrics.LyricsState
import com.perseusoft.homefi.ui.theme.LyricsPalette
import com.perseusoft.homefi.ui.theme.rememberCoverAccent
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Where the now-playing cover sits in the window, so the system can animate the cover into
 * the picture-in-picture window. Null when the player isn't on screen.
 */
object PipSourceRect {
    val bounds = MutableStateFlow<Rect?>(null)
}

/** Reports this composable's window bounds as the picture-in-picture source rect. */
fun Modifier.pipSourceRect(): Modifier = this.then(
    Modifier.onGloballyPositioned { coords ->
        val r = coords.boundsInWindow()
        PipSourceRect.bounds.value = Rect(r.left.toInt(), r.top.toInt(), r.right.toInt(), r.bottom.toInt())
    },
)

private val PipInk = Color.White
private val PipDim = Color.White.copy(alpha = 0.6f)

/**
 * The compact player inside the picture-in-picture window, over everything else: the cover
 * with title and artist, or (each its own setting) the synced karaoke lines or HomeFito
 * dancing. No controls here; the window's own buttons do previous / play-pause / next.
 */
@Composable
fun PipPlayer(modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val state by app.player.state.collectAsStateWithLifecycle()
    val now = state.current
    val mediaId = now?.mediaId
    val cover = now?.coverModel()
    val accent = rememberCoverAccent(cover) ?: MaterialTheme.colorScheme.primary
    LaunchedEffect(mediaId, settings.pipLyrics) {
        if (mediaId != null && settings.pipLyrics) app.lyrics.request(mediaId)
    }
    val lyricsStates by app.lyrics.states.collectAsStateWithLifecycle()
    val lyrics = (lyricsStates[mediaId] as? LyricsState.Ready)?.lyrics?.takeIf { it.synced && settings.pipLyrics }

    Box(modifier.fillMaxSize().background(Color.Black)) {
        when {
            lyrics != null -> {
                CoverArt(cover, size = null, radius = 0.dp, modifier = Modifier.fillMaxSize())
                Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.72f)))
                val palette = LyricsPalette(ink = PipInk, dim = PipDim, fill = accent, fillEnd = PipInk)
                PipLyrics(lyrics, state, palette, settings.lyricsOffsetMs)
            }
            settings.pipMascot -> Column(
                Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).padding(6.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                MascotView(Modifier.weight(1f).fillMaxWidth(), singing = false, danceButton = false)
                now?.let { PipTitle(it.title, it.artist, MaterialTheme.colorScheme.onBackground) }
            }
            else -> {
                CoverArt(cover, size = null, radius = 0.dp, modifier = Modifier.fillMaxSize())
                Box(
                    Modifier.fillMaxSize().background(
                        Brush.verticalGradient(0.55f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.8f)),
                    ),
                )
                now?.let {
                    Column(Modifier.align(Alignment.BottomCenter).padding(8.dp)) { PipTitle(it.title, it.artist, PipInk) }
                }
            }
        }
    }
}

@Composable
private fun PipTitle(title: String, artist: String, color: Color) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            title,
            color = color,
            fontSize = 13.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
            style = MaterialTheme.typography.titleSmall,
        )
        if (artist.isNotBlank()) {
            Text(
                artist,
                color = color.copy(alpha = 0.7f),
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}
