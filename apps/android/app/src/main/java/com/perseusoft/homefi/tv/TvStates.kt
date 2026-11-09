package com.perseusoft.homefi.tv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// Empty, "coming soon" and loading states: one glass card, an accent line icon, a title,
// one sentence and one primary pill that already has focus (docs/TV_UI.md 6.12).

/** Glass card with an icon medallion, [title], [body] and a focused call to action. */
@Composable
fun TvEmptyState(
    title: String,
    body: String,
    cta: String,
    ctaIcon: ImageVector?,
    entryKey: Int,
    onCta: () -> Unit,
    icon: ImageVector? = null,
    tag: String? = null,
    modifier: Modifier = Modifier,
) {
    val look = LocalTvLook.current
    TvGlass(modifier.width(300.dp), radius = 14.dp) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 22.dp, vertical = 22.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (icon != null) {
                Box(
                    Modifier
                        .size(52.dp)
                        .background(look.accent.copy(alpha = 0.16f), CircleShape)
                        .border(1.5.dp, look.accent.copy(alpha = 0.7f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) { Icon(icon, null, tint = look.accent2, modifier = Modifier.size(28.dp)) }
            }
            if (tag != null) TvTag(tag, tone = look.accent2)
            Text(title, style = TvType.section.copy(fontSize = 20.sp), color = look.ink, textAlign = TextAlign.Center)
            Text(body, style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink2, textAlign = TextAlign.Center)
            TvPill(cta, onCta, Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(), icon = ctaIcon, primary = true)
        }
    }
}

/** The shape of what is coming: a few calm blocks while the account is read. */
@Composable
fun TvLoading(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Box(Modifier.size(width = 220.dp, height = 8.dp).background(Color.White.copy(alpha = 0.10f), CircleShape))
    }
}

/** The panel of a section that is not built yet: says what is coming, with one pill back to Home. */
@Composable
fun TvComingSoon(section: TvSection, entryKey: Int, onHome: () -> Unit, note: String? = null, modifier: Modifier = Modifier) {
    val label = androidx.compose.ui.res.stringResource(section.label)
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        TvEmptyState(
            title = if (note != null) label else androidx.compose.ui.res.stringResource(com.perseusoft.homefi.R.string.tv_soon_title, label),
            body = note ?: androidx.compose.ui.res.stringResource(com.perseusoft.homefi.R.string.tv_soon_body),
            cta = androidx.compose.ui.res.stringResource(com.perseusoft.homefi.R.string.tv_soon_back),
            ctaIcon = androidx.compose.material.icons.Icons.Rounded.Home,
            entryKey = entryKey,
            onCta = onHome,
            icon = section.icon,
            tag = androidx.compose.ui.res.stringResource(com.perseusoft.homefi.R.string.tv_soon_tag),
        )
    }
}
