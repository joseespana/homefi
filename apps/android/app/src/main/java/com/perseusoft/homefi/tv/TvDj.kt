package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Home
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

/**
 * The DJ section (docs/TV_APP_PLAN.md, Decisions): the phone's two-deck screen runs as it is over the TV,
 * and its buttons, faders and pads are made for a pointer, so it needs a mouse. With one (or `tv.djEntry` on)
 * this page opens it; without, it says so. There is no D-pad DJ.
 */
@Composable
fun TvDj(entryKey: Int, usable: Boolean, onHome: () -> Unit) {
    val app = LocalContext.current.homefi
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        if (usable) {
            TvEmptyState(
                title = stringResource(R.string.tv_nav_dj),
                body = stringResource(R.string.tv_dj_ready_body),
                cta = stringResource(R.string.tv_dj_open),
                ctaIcon = Icons.Rounded.Album,
                entryKey = entryKey,
                onCta = app.dj::open,
                icon = Icons.Rounded.Album,
            )
        } else {
            TvEmptyState(
                title = stringResource(R.string.tv_dj_need_mouse),
                body = stringResource(R.string.tv_dj_need_mouse_body),
                cta = stringResource(R.string.tv_soon_back),
                ctaIcon = Icons.Rounded.Home,
                entryKey = entryKey,
                onCta = onHome,
                icon = Icons.Rounded.Album,
            )
        }
    }
}
