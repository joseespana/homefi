package com.perseusoft.homefi.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.ParagraphStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextIndent
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.NoteLine
import com.perseusoft.homefi.core.ReleaseNotes
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** Share of the screen height the notes may take, so the button stays visible on small screens. */
private const val BODY_HEIGHT_FRACTION = 0.55f
private val SECTION_GAP = 12.dp
private val VERSION_GAP = 16.dp
private val BULLET_INDENT = 16.sp

/** "What's new" after an update, or reopened from Settings → About: text only, no WebView. */
@Composable
fun ReleaseNotesDialog(notes: List<ReleaseNotes>, onDismiss: () -> Unit) {
    val haptics = haptics()
    val maxBody = (LocalConfiguration.current.screenHeightDp * BODY_HEIGHT_FRACTION).dp
    val single = notes.singleOrNull()
    val close = { haptics.tick(); onDismiss() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                if (single != null) stringResource(R.string.update_whats_new_title, single.version)
                else stringResource(R.string.update_whats_new),
            )
        },
        text = {
            Column(
                Modifier.heightIn(max = maxBody).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(VERSION_GAP),
            ) {
                if (notes.isEmpty()) {
                    Text(
                        stringResource(R.string.whats_new_none),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                notes.forEachIndexed { index, release ->
                    if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    VersionNotes(release, showVersion = single == null)
                }
            }
        },
        confirmButton = { TextButton(onClick = close, modifier = Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.whats_new_close)) } },
    )
}

@Composable
private fun VersionNotes(release: ReleaseNotes, showVersion: Boolean) {
    val date = release.date.takeIf { it.isNotBlank() }?.let(::formatDate)
    Column(verticalArrangement = Arrangement.spacedBy(SECTION_GAP)) {
        if (showVersion) {
            Text(
                if (date != null) stringResource(R.string.whats_new_version, release.version, date) else "HomeFi ${release.version}",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.semantics { heading() },
            )
        } else if (date != null) {
            Text(date, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        release.summary?.let { Text(it.toAnnotated(), style = MaterialTheme.typography.bodyMedium) }
        release.sections.forEach { section ->
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    section.title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.semantics { heading() },
                )
                section.items.forEach { item ->
                    Text(bullet(item), style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}

/** "3 Nov 2026" in the user's language; the raw text when it isn't an ISO date. */
private fun formatDate(iso: String): String = runCatching {
    val parts = iso.split('-').map(String::toInt)
    val day = java.util.GregorianCalendar(parts[0], parts[1] - 1, parts[2]).time
    DateFormat.getDateInstance(DateFormat.MEDIUM, Locale.getDefault()).format(Date(day.time))
}.getOrDefault(iso)

/** A bullet with a hanging indent, so wrapped lines align under the text. */
@Composable
private fun bullet(line: NoteLine): AnnotatedString {
    val body = line.toAnnotated()
    return buildAnnotatedString {
        withStyle(ParagraphStyle(textIndent = TextIndent(firstLine = 0.sp, restLine = BULLET_INDENT))) {
            append("•  ")
            append(body)
        }
    }
}

/** Spans to styled text: bold, code chip, links that open the system browser. */
@Composable
private fun NoteLine.toAnnotated(): AnnotatedString {
    val linkStyle = TextLinkStyles(SpanStyle(color = MaterialTheme.colorScheme.primary, textDecoration = TextDecoration.Underline))
    val codeBackground = MaterialTheme.colorScheme.surfaceVariant
    return buildAnnotatedString {
        spans.forEach { span ->
            val style = SpanStyle(
                fontWeight = if (span.bold) FontWeight.SemiBold else null,
                fontFamily = if (span.code) FontFamily.Monospace else null,
                background = if (span.code) codeBackground else androidx.compose.ui.graphics.Color.Unspecified,
            )
            val link = span.link
            if (link != null) {
                withLink(LinkAnnotation.Url(link, linkStyle)) { withStyle(style) { append(span.text) } }
            } else {
                withStyle(style) { append(span.text) }
            }
        }
    }
}
