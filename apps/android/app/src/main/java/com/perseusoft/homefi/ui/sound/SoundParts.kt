package com.perseusoft.homefi.ui.sound

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.TasteCreator
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.QualityBadge
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.FilledTonalButton
import com.perseusoft.homefi.player.name

/** Height of every tappable row and the smallest touch target (dp), as the design asks. */
val TOUCH_TARGET = 48.dp

/** "Song", "Album", "Artist", "Playlist", "Genre". */
@Composable
fun TasteScope.kindLabel(): String = stringResource(
    when (this) {
        is TasteScope.Track -> R.string.sound_kind_song
        is TasteScope.Album -> R.string.sound_kind_album
        is TasteScope.Artist -> R.string.sound_kind_artist
        is TasteScope.Playlist -> R.string.sound_kind_playlist
        is TasteScope.Genre -> R.string.sound_kind_genre
    },
)

/** Section titles of "By music", in the order the core lists scopes. */
val KIND_ORDER: List<kotlin.reflect.KClass<out TasteScope>> = listOf(
    TasteScope.Track::class, TasteScope.Album::class, TasteScope.Artist::class, TasteScope.Playlist::class, TasteScope.Genre::class,
)

@Composable
fun kindsLabel(kind: kotlin.reflect.KClass<out TasteScope>): String = stringResource(
    when (kind) {
        TasteScope.Track::class -> R.string.sound_kinds_songs
        TasteScope.Album::class -> R.string.sound_kinds_albums
        TasteScope.Artist::class -> R.string.sound_kinds_artists
        TasteScope.Playlist::class -> R.string.sound_kinds_playlists
        else -> R.string.sound_kinds_genres
    },
)

/** A small rounded square with the kind's icon: where a target comes from at a glance. */
@Composable
fun KindBadge(scope: TasteScope?, modifier: Modifier = Modifier, empty: Boolean = false) {
    val scheme = MaterialTheme.colorScheme
    Surface(shape = RoundedCornerShape(8.dp), color = scheme.secondaryContainer, contentColor = scheme.onSecondaryContainer, modifier = modifier.size(36.dp)) {
        Box(contentAlignment = Alignment.Center) {
            if (scope == null && empty) {
                Icon(Icons.Rounded.Layers, null, Modifier.size(20.dp))
            } else if (scope == null) {
                Text(stringResource(R.string.sound_badge_all), style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
            } else {
                Icon(scope.icon(), null, Modifier.size(20.dp))
            }
        }
    }
}

/** The "AI" tag of a sound the AI made. */
@Composable
fun AiTag(rule: TasteRule, modifier: Modifier = Modifier) {
    if (rule.createdBy == TasteCreator.AI) QualityBadge(stringResource(R.string.sound_ai_tag), modifier)
}

/** "Eagles, Fleetwood Mac +1": what a sound applies to, at a glance. */
fun targetsSummary(rule: TasteRule, shown: Int = 2): String {
    val names = rule.targets.map { it.scope.name() }
    val extra = (names.size - shown).coerceAtLeast(0)
    return names.take(shown).joinToString(", ") + if (extra > 0) " +$extra" else ""
}

/** A proactive suggestion: nothing changes before a yes, which goes on in the assistant. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SuggestionCard(text: String?, replies: List<String>, onReply: (String) -> Unit, onHide: () -> Unit) {
    Card(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(Modifier.padding(start = 12.dp, top = 12.dp, bottom = 12.dp), verticalAlignment = Alignment.Top) {
            Icon(AiIcon, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 2.dp))
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.sound_suggestion), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                if (text == null) {
                    CircularProgressIndicator(Modifier.padding(vertical = 8.dp).size(20.dp), strokeWidth = 2.dp)
                } else {
                    Text(text, style = MaterialTheme.typography.bodyMedium)
                    if (replies.isNotEmpty()) {
                        FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            replies.forEachIndexed { i, r ->
                                if (i == 0) Button(onClick = { onReply(r) }) { Text(r) } else FilledTonalButton(onClick = { onReply(r) }) { Text(r) }
                            }
                        }
                    }
                }
            }
            IconButton(onClick = onHide) { Icon(Icons.Rounded.Close, stringResource(R.string.sound_suggestion_hide)) }
        }
    }
}
