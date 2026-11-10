package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.ui.unit.sp
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

/** Where a screen can send the user: another section of the drawer, or the full-screen player. */
class TvNav(
    val go: (TvSection) -> Unit,
    val openPlayer: () -> Unit,
    /** Settings, opened on one page (the AI page from "set up an AI"). */
    val openSettings: (TvSettingsCategory) -> Unit = {},
)

val LocalTvNav = staticCompositionLocalOf { TvNav(go = {}, openPlayer = {}) }

/**
 * "Now playing: <title>" as a pill in a screen's header: one OK from any screen back to the player,
 * only while something is loaded.
 */
@Composable
fun TvNowPlayingChip(modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val state by app.player.state.collectAsStateWithLifecycle()
    val current = state.current ?: return
    val nav = LocalTvNav.current
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        TvPill(
            current.title,
            nav.openPlayer,
            Modifier.width(MAX_CHIP),
            icon = Icons.Rounded.GraphicEq,
            height = 30.dp,
            labelStyle = TvType.body.copy(fontSize = 13.sp),
            description = stringResource(R.string.tv_np_open),
        )
    }
}

private val MAX_CHIP = 210.dp
