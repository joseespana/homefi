package com.perseusoft.homefi.ui.share

import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.runtime.remember
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

/** Colours people can pick for themselves (shown next to their songs). */
internal val SharePalette = listOf(
    "#0891B2", "#1D4ED8", "#10B981", "#F59E0B", "#EF4444", "#EC4899", "#8B5CF6", "#64748B",
)

/** A member's `#RRGGBB`, or the theme's primary when it can't be read. */
@Composable
internal fun memberColor(hex: String?): Color {
    val fallback = MaterialTheme.colorScheme.primary
    return hex?.let { runCatching { Color(android.graphics.Color.parseColor(it)) }.getOrNull() } ?: fallback
}

@Composable
internal fun MemberDot(hex: String?, size: Dp = 12.dp) {
    Box(Modifier.size(size).clip(CircleShape).background(memberColor(hex)))
}

/** Pick your colour: big round swatches. */
@Composable
internal fun ColorPicker(selected: String, onPick: (String) -> Unit, modifier: Modifier = Modifier) {
    Row(modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        SharePalette.forEach { hex ->
            val isSelected = hex.equals(selected, ignoreCase = true)
            Box(
                Modifier
                    .size(40.dp)
                    .clip(CircleShape)
                    .background(memberColor(hex))
                    .border(if (isSelected) 3.dp else 0.dp, MaterialTheme.colorScheme.onSurface, CircleShape)
                    .clickable(role = Role.RadioButton, onClickLabel = hex) { onPick(hex) },
                contentAlignment = Alignment.Center,
            ) {
                if (isSelected) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.surface)
            }
        }
    }
}

/**
 * The hub's discreet notices ("Ana added …"): small pills at the top that
 * fade away by themselves; a tap hides one. Nothing to answer, no sound.
 */
@Composable
fun ShareNotices(modifier: Modifier = Modifier) {
    val share = LocalContext.current.homefi.share
    val notices by share.notices.collectAsStateWithLifecycle()
    Column(
        modifier.fillMaxWidth().statusBarsPadding().padding(top = 64.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        notices.forEach { n ->
            androidx.compose.runtime.key(n.id) {
                // fades in when it arrives; removal is instant (it already timed out)
                val shown = remember { MutableTransitionState(false).apply { targetState = true } }
                val motion = LocalMotion.current
                AnimatedVisibility(
                    visibleState = shown,
                    enter = if (motion.moves) fadeIn(motion.tween(200)) + slideInVertically(motion.move()) else fadeIn(motion.tween(200)),
                    exit = fadeOut(motion.tween(150)),
                ) {
                    Surface(
                        shape = RoundedCornerShape(50),
                        color = MaterialTheme.colorScheme.inverseSurface.copy(alpha = 0.92f),
                        contentColor = MaterialTheme.colorScheme.inverseOnSurface,
                        shadowElevation = 4.dp,
                        modifier = Modifier.widthIn(max = 420.dp).padding(horizontal = 24.dp).clickable(onClickLabel = stringResource(R.string.sharing_notice_hide)) {
                            share.dismissNotice(n.id)
                        },
                    ) {
                        Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                            MemberDot(n.color, 10.dp)
                            Spacer(Modifier.width(8.dp))
                            Text(n.text, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }
        }
    }
}
