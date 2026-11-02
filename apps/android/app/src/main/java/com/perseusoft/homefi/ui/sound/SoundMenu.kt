package com.perseusoft.homefi.ui.sound

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.tasteSoundCovering
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.haptics

/**
 * "Sound ▸" (long-press or the Sound button on an artist, album, playlist or song): which
 * sound plays for [scope]. A check marks the sound that holds it now; "Everything else"
 * takes it out of its sound. With [title] set it is "Move to sound".
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SoundMenu(scope: TasteScope, onDismiss: () -> Unit, title: String? = null) {
    val app = LocalContext.current.homefi
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val output by app.equalizer.outputId.collectAsStateWithLifecycle()
    val haptics = haptics()
    val holder = remember(taste, scope, output) { runCatching { tasteSoundCovering(taste, scope, output) }.getOrNull()?.id }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 12.dp)) {
            Text(
                title ?: stringResource(R.string.sound_for_item, scope.name()),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            taste.rules.forEach { rule ->
                MenuRow(
                    checked = rule.id == holder,
                    text = rule.label,
                    detail = targetsSummary(rule),
                    onClick = { haptics.tick(); app.sound.assign(rule.id, scope); onDismiss() },
                )
            }
            MenuRow(
                checked = holder == null,
                text = stringResource(R.string.sound_everything_else),
                detail = null,
                onClick = { haptics.tick(); holder?.let { app.sound.unassign(it, scope) }; onDismiss() },
            )
            MenuRow(
                checked = false,
                text = stringResource(R.string.sound_new_from_ai),
                detail = null,
                icon = { Icon(AiIcon, null, tint = MaterialTheme.colorScheme.primary) },
                onClick = { haptics.press(); onDismiss(); app.sound.askAssistant(scope) },
            )
            MenuRow(
                checked = false,
                text = stringResource(R.string.sound_new),
                detail = null,
                icon = { Icon(Icons.Rounded.Add, null, tint = MaterialTheme.colorScheme.primary) },
                onClick = { haptics.press(); app.sound.newSound(listOf(scope)); onDismiss() },
            )
        }
    }
}

@Composable
private fun MenuRow(checked: Boolean, text: String, detail: String?, onClick: () -> Unit, icon: (@Composable () -> Unit)? = null) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = TOUCH_TARGET).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.size(24.dp), verticalAlignment = Alignment.CenterVertically) {
            when {
                checked -> Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary)
                icon != null -> icon()
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(text, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (!detail.isNullOrBlank()) Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}
