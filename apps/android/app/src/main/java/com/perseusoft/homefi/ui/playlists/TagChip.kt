package com.perseusoft.homefi.ui.playlists

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EntryTag

private fun EntryTag.labelRes(): Int? = when (this) {
    EntryTag.ALWAYS -> R.string.pl_tag_always
    EntryTag.USUALLY -> R.string.pl_tag_usually
    EntryTag.SOMETIMES -> R.string.pl_tag_sometimes
    EntryTag.ENCORE -> R.string.pl_tag_encore
    EntryTag.PLAIN -> null
}

/** "Always" / "Usually" / "Sometimes" / "Encore" as a small chip; nothing for a plain song. */
@Composable
fun TagChip(tag: EntryTag, modifier: Modifier = Modifier) {
    val label = tag.labelRes() ?: return
    val scheme = MaterialTheme.colorScheme
    val (container, content) = when (tag) {
        EntryTag.ALWAYS -> scheme.primaryContainer to scheme.onPrimaryContainer
        EntryTag.USUALLY -> scheme.secondaryContainer to scheme.onSecondaryContainer
        EntryTag.ENCORE -> scheme.tertiaryContainer to scheme.onTertiaryContainer
        else -> scheme.surfaceContainerHighest to scheme.onSurfaceVariant
    }
    Chip(stringResource(label), container, content, modifier)
}

/** The chip on a song neither Tidal nor the library has. */
@Composable
fun NotFoundChip(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Chip(stringResource(R.string.pl_not_found), scheme.errorContainer, scheme.onErrorContainer, modifier)
}

/** The "Not the real setlist" badge, loud on purpose. */
@Composable
fun NotRealSetlistBadge(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Chip(stringResource(R.string.pl_not_real_setlist), scheme.tertiaryContainer, scheme.onTertiaryContainer, modifier, bold = true)
}

/** The "Party" badge on a playlist made by the party screen. */
@Composable
fun PartyBadge(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Chip(stringResource(R.string.pl_party_badge), scheme.secondaryContainer, scheme.onSecondaryContainer, modifier)
}

/** The "Chat" badge on a playlist made in the "Playlist with AI" chat. */
@Composable
fun ChatBadge(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Chip(stringResource(R.string.pl_chat_badge), scheme.tertiaryContainer, scheme.onTertiaryContainer, modifier)
}

@Composable
private fun Chip(
    text: String,
    container: androidx.compose.ui.graphics.Color,
    content: androidx.compose.ui.graphics.Color,
    modifier: Modifier,
    bold: Boolean = false,
) {
    Surface(modifier, shape = RoundedCornerShape(8.dp), color = container, contentColor = content) {
        Text(
            text,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = if (bold) FontWeight.Bold else FontWeight.Medium,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
        )
    }
}
