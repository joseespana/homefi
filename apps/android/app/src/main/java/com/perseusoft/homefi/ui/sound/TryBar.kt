package com.perseusoft.homefi.ui.sound

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.OutlinedButton
import com.perseusoft.homefi.core.TasteAudition
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

private const val SLIDE_MS = 150

/**
 * "Trying “Warm” on this song": pinned on top while a sound is tried. Keep saves it for the
 * scope the core suggests (primary) or another one of the song; Undo goes back. It ends by
 * itself with the song.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TryBar(modifier: Modifier = Modifier) {
    val app = LocalContext.current.homefi
    val audition by app.sound.audition.collectAsStateWithLifecycle()
    val now by app.sound.now.collectAsStateWithLifecycle()
    val haptics = haptics()
    val motion = LocalMotion.current
    // the last one stays drawn while the bar slides away
    var last by remember { mutableStateOf<TasteAudition?>(null) }
    if (audition != null) last = audition
    val shown = last
    AnimatedVisibility(
        visible = audition != null,
        enter = if (motion.enabled) expandVertically(tween(SLIDE_MS)) + fadeIn(tween(SLIDE_MS)) else expandVertically(tween(0)),
        exit = if (motion.enabled) shrinkVertically(tween(SLIDE_MS)) + fadeOut(tween(SLIDE_MS)) else shrinkVertically(tween(0)),
        modifier = modifier,
    ) {
        val rule = (shown ?: return@AnimatedVisibility).rule
        val primary = remember(rule, now) { app.sound.suggestedScope(rule) }
        val scheme = MaterialTheme.colorScheme
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = scheme.secondaryContainer,
            contentColor = scheme.onSecondaryContainer,
            border = BorderStroke(1.dp, scheme.primary),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            Column(Modifier.padding(12.dp)) {
                Text(
                    stringResource(R.string.sound_trying_on, rule.label),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                FlowRow(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (primary != null) {
                        Button(onClick = { haptics.confirm(); app.sound.keep(primary) }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) {
                            Text(stringResource(R.string.sound_keep_for, primary.kindLabel().lowercase()))
                        }
                    }
                    now.scopes.filter { it != primary }.forEach { s ->
                        OutlinedButton(onClick = { haptics.confirm(); app.sound.keep(s) }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) {
                            Text(s.kindLabel())
                        }
                    }
                    TextButton(onClick = { haptics.tick(); app.sound.stopTrying() }, modifier = Modifier.heightIn(min = TOUCH_TARGET)) {
                        Text(stringResource(R.string.sound_undo))
                    }
                }
            }
        }
    }
}
