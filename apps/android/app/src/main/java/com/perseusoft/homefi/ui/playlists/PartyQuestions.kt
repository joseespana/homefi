package com.perseusoft.homefi.ui.playlists

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AgeGroup
import com.perseusoft.homefi.core.EnergyCurve
import com.perseusoft.homefi.core.PartyBrief
import com.perseusoft.homefi.core.PartyGuests
import com.perseusoft.homefi.core.PartyVibe
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.settings.Segmented

// Ranges mirror the core's party constants; the core clamps again when it builds.
internal const val MIN_PARTY_MINUTES = 30
internal const val MAX_PARTY_MINUTES = 480
internal const val PARTY_MINUTES_STEP = 15
private const val MAX_NOTES_CHARS = 500
private const val MINUTES_PER_HOUR = 60
private const val NOTES_MAX_LINES = 3

/** "2 h", "1 h 30 min", "45 min": the same words in every language. */
@Composable
internal fun partyLength(minutes: Int): String {
    val hours = minutes / MINUTES_PER_HOUR
    val rest = minutes % MINUTES_PER_HOUR
    return when {
        hours == 0 -> stringResource(R.string.pl_party_len_m, rest)
        rest == 0 -> stringResource(R.string.pl_party_len_h, hours)
        else -> stringResource(R.string.pl_party_len_hm, hours, rest)
    }
}

@Composable
private fun guestsLabel(guests: PartyGuests) = stringResource(
    when (guests) {
        PartyGuests.FRIENDS -> R.string.pl_party_guests_friends
        PartyGuests.FAMILY -> R.string.pl_party_guests_family
        PartyGuests.COWORKERS -> R.string.pl_party_guests_coworkers
        PartyGuests.MIXED -> R.string.pl_party_guests_mixed
    },
)

@Composable
private fun vibeLabel(vibe: PartyVibe) = stringResource(
    when (vibe) {
        PartyVibe.BACKGROUND -> R.string.pl_party_vibe_background
        PartyVibe.CHILL -> R.string.pl_party_vibe_chill
        PartyVibe.DANCE -> R.string.pl_party_vibe_dance
        PartyVibe.SINGALONG -> R.string.pl_party_vibe_singalong
        PartyVibe.THROWBACK -> R.string.pl_party_vibe_throwback
    },
)

@Composable
private fun ageLabel(age: AgeGroup) = stringResource(
    when (age) {
        AgeGroup.KIDS -> R.string.pl_party_age_kids
        AgeGroup.TEENS -> R.string.pl_party_age_teens
        AgeGroup.YOUNG_ADULTS -> R.string.pl_party_age_young
        AgeGroup.ADULTS -> R.string.pl_party_age_adults
        AgeGroup.SENIORS -> R.string.pl_party_age_seniors
    },
)

@Composable
internal fun curveLabel(curve: EnergyCurve) = stringResource(
    when (curve) {
        EnergyCurve.ARC -> R.string.pl_party_curve_arc
        EnergyCurve.RISING -> R.string.pl_party_curve_rising
        EnergyCurve.STEADY -> R.string.pl_party_curve_steady
    },
)

/** The energy choices as wrapping chips: their labels are too long for a segmented strip. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CurveChips(selected: EnergyCurve, enabled: Boolean, onSelect: (EnergyCurve) -> Unit) {
    FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        EnergyCurve.entries.forEach { curve -> PartyChip(selected == curve, enabled, curveLabel(curve)) { onSelect(curve) } }
    }
}

/** "Friends · Dance · 2 h": the answers in one line, for when the questions fold away. */
@Composable
internal fun briefSummary(brief: PartyBrief): String =
    listOf(guestsLabel(brief.guests), vibeLabel(brief.vibe), partyLength(brief.minutes.toInt())).joinToString(" · ")

/** "Any age · Warm up, peak, wind down · Explicit allowed": what "More details" holds. */
@Composable
private fun detailsSummary(brief: PartyBrief): String {
    val ages = if (brief.ages.isEmpty()) stringResource(R.string.pl_party_summary_ages_any) else brief.ages.map { ageLabel(it) }.joinToString(", ")
    val explicit = stringResource(if (brief.allowExplicit) R.string.pl_party_explicit_summary_allow else R.string.pl_party_explicit_summary_avoid)
    return listOf(ages, curveLabel(brief.curve), explicit).joinToString(" · ")
}

/** The three questions that matter, with the other four folded under "More details". */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun PartyQuestions(
    brief: PartyBrief,
    enabled: Boolean,
    moreOpen: Boolean,
    onToggleMore: () -> Unit,
    onBrief: (PartyBrief) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Question(stringResource(R.string.pl_party_q_guests)) {
            FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PartyGuests.entries.forEach { guests ->
                    PartyChip(brief.guests == guests, enabled, guestsLabel(guests)) { onBrief(brief.copy(guests = guests)) }
                }
            }
        }
        Question(stringResource(R.string.pl_party_q_vibe)) {
            FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PartyVibe.entries.forEach { vibe ->
                    PartyChip(brief.vibe == vibe, enabled, vibeLabel(vibe)) { onBrief(brief.copy(vibe = vibe)) }
                }
            }
        }
        LengthStepper(brief.minutes.toInt(), enabled) { onBrief(brief.copy(minutes = it.toUShort())) }
        MoreDetails(brief, enabled, moreOpen, onToggleMore, onBrief)
    }
}

@Composable
private fun Question(title: String, hint: String? = null, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        if (hint != null) {
            Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        content()
    }
}

/** A choice chip; the check icon says "picked" without relying on colour. */
@Composable
private fun PartyChip(selected: Boolean, enabled: Boolean, label: String, onClick: () -> Unit) {
    SelectChip(
        selected = selected,
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.heightIn(min = MIN_TOUCH_DP.dp),
        label = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (selected) {
                    Icon(Icons.Rounded.Check, null, Modifier.size(16.dp))
                    Spacer(Modifier.size(4.dp))
                }
                Text(label)
            }
        },
    )
}

/** Minus and plus in 15 minute steps: a thumb over 30 to 480 minutes would be imprecise. */
@Composable
private fun LengthStepper(minutes: Int, enabled: Boolean, onChange: (Int) -> Unit) {
    val haptics = haptics()
    val readout = partyLength(minutes)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.pl_party_q_length), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            Text(
                readout,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
        IconButton(
            onClick = { haptics.press(); onChange((minutes - PARTY_MINUTES_STEP).coerceAtLeast(MIN_PARTY_MINUTES)) },
            enabled = enabled && minutes > MIN_PARTY_MINUTES,
        ) { Icon(Icons.Rounded.Remove, stringResource(R.string.pl_party_length_shorter)) }
        IconButton(
            onClick = { haptics.press(); onChange((minutes + PARTY_MINUTES_STEP).coerceAtMost(MAX_PARTY_MINUTES)) },
            enabled = enabled && minutes < MAX_PARTY_MINUTES,
        ) { Icon(Icons.Rounded.Add, stringResource(R.string.pl_party_length_longer)) }
    }
}

@OptIn(ExperimentalLayoutApi::class, ExperimentalFoundationApi::class)
@Composable
private fun MoreDetails(
    brief: PartyBrief,
    enabled: Boolean,
    open: Boolean,
    onToggle: () -> Unit,
    onBrief: (PartyBrief) -> Unit,
) {
    val title = stringResource(if (open) R.string.pl_party_less else R.string.pl_party_more)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(onClick = onToggle)
            .semantics { stateDescription = title },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.pl_party_more), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            if (!open) {
                Text(detailsSummary(brief), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Icon(if (open) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore, null)
    }
    if (!open) return
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Question(stringResource(R.string.pl_party_q_ages), hint = stringResource(R.string.pl_party_q_ages_hint)) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AgeGroup.entries.forEach { age ->
                    val picked = age in brief.ages
                    PartyChip(picked, enabled, ageLabel(age)) {
                        // keep the core's order whatever the tap order was
                        onBrief(brief.copy(ages = AgeGroup.entries.filter { (it in brief.ages) != (it == age) }))
                    }
                }
            }
        }
        Question(stringResource(R.string.pl_party_q_curve)) {
            CurveChips(brief.curve, enabled) { onBrief(brief.copy(curve = it)) }
        }
        val explicitTitle = stringResource(R.string.pl_party_q_explicit)
        Question(explicitTitle) {
            Segmented(
                listOf(true, false),
                brief.allowExplicit,
                { stringResource(if (it) R.string.pl_party_explicit_allow else R.string.pl_party_explicit_avoid) },
                { onBrief(brief.copy(allowExplicit = it)) },
                description = explicitTitle,
            )
        }
        OutlinedTextField(
            value = brief.notes,
            onValueChange = { onBrief(brief.copy(notes = it.take(MAX_NOTES_CHARS))) },
            modifier = Modifier.fillMaxWidth().padding(bottom = 4.dp),
            enabled = enabled,
            label = { Text(stringResource(R.string.pl_party_q_notes)) },
            placeholder = { Text(stringResource(R.string.pl_party_q_notes_hint)) },
            maxLines = NOTES_MAX_LINES,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
        )
    }
}

/** True when an answer folded under "More details" differs from the Settings defaults. */
internal fun PartyBrief.detailsDiffer(defaults: PartyBrief): Boolean =
    ages.isNotEmpty() || notes.isNotBlank() || curve != defaults.curve || allowExplicit != defaults.allowExplicit
