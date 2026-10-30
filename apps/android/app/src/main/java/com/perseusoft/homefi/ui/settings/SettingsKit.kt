package com.perseusoft.homefi.ui.settings

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.SkinHeading

// Building blocks of the Settings pages; the desktop has the same ones (SettingsGroup,
// SettingRow, SwitchRow, Segmented, ChoiceList, OptionGallery). Pages compose these and
// never draw their own rows, so a new look is one change here.

/** Size classes of the Settings screen, by the width it is given (not the device). */
internal val COMPACT_MAX_WIDTH = 600.dp
internal val EXPANDED_MIN_WIDTH = 840.dp
internal val NAV_PANE_WIDTH = 280.dp

/** Content column widths for "Settings width: Comfortable / Wide". */
internal val COMFORTABLE_CONTENT_WIDTH = 640.dp
internal val WIDE_CONTENT_WIDTH = 840.dp

private val ROW_MIN_HEIGHT = 56.dp
private val ROW_PADDING_H = 16.dp
private val ROW_PADDING_V = 8.dp
internal val GROUP_MARGIN_H = 16.dp
private val GROUP_GAP = 16.dp
private val GALLERY_CARD_MIN = 150.dp
private val GALLERY_GAP = 12.dp
private const val SEGMENTED_MAX_OPTIONS = 4
/** Narrowest a segmented button can be before a label like "Automático" gets cut. */
private val SEGMENT_MIN_WIDTH = 92.dp
private const val DISABLED_ALPHA = 0.38f

/**
 * A card of related rows with its caption above. Children are separated by hairlines the
 * page does not draw itself; the card uses the skin's shape, frame and glass.
 */
@Composable
internal fun SettingsGroup(
    modifier: Modifier = Modifier,
    title: String? = null,
    description: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .fillMaxWidth()
            .padding(horizontal = GROUP_MARGIN_H, vertical = GROUP_GAP / 2),
    ) {
        if (title != null) {
            SkinHeading(
                title,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .padding(start = 4.dp, bottom = 4.dp)
                    .semantics { heading() },
            )
        }
        if (description != null) {
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, bottom = 8.dp),
            )
        }
        Card(
            modifier = Modifier.fillMaxWidth(),
            shape = MaterialTheme.shapes.large,
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        ) {
            Column(Modifier.padding(vertical = 4.dp), content = content)
        }
    }
}

/** A hairline between two rows of a group. */
@Composable
internal fun RowDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(horizontal = ROW_PADDING_H),
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f),
    )
}

/**
 * Label (and hint) on the left, a control on the right; at least 56 dp tall. A row with
 * [onClick] is clickable as a whole. A disabled row dims its text.
 */
@Composable
internal fun SettingRow(
    label: String,
    modifier: Modifier = Modifier,
    hint: String? = null,
    enabled: Boolean = true,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = ROW_MIN_HEIGHT)
            .padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(ROW_PADDING_H),
    ) {
        RowText(label, hint, enabled, Modifier.weight(1f))
        trailing?.invoke()
    }
}

@Composable
private fun RowText(label: String, hint: String?, enabled: Boolean, modifier: Modifier = Modifier) {
    val ink by animateColorAsState(
        if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurface.copy(alpha = DISABLED_ALPHA),
        LocalMotion.current.color(),
        label = "row-ink",
    )
    Column(modifier) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = ink)
        if (hint != null) {
            Text(
                hint,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = if (enabled) 1f else DISABLED_ALPHA + 0.1f),
            )
        }
    }
}

/** A setting whose control is wide (slider, segmented, field): label above, control below. */
@Composable
internal fun StackedSetting(
    label: String,
    modifier: Modifier = Modifier,
    hint: String? = null,
    enabled: Boolean = true,
    control: @Composable () -> Unit,
) {
    Column(
        modifier
            .fillMaxWidth()
            .heightIn(min = ROW_MIN_HEIGHT)
            .padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        RowText(label, null, enabled)
        control()
        if (hint != null) {
            AnimatedHint(hint)
        }
    }
}

/** A hint whose text fades when the choice above it changes it. */
@Composable
internal fun AnimatedHint(hint: String, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    AnimatedContent(targetState = hint, transitionSpec = { motion.swap() }, label = "hint", modifier = modifier) { shown ->
        Text(
            shown,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** A switch row: the whole row toggles, with a haptic tick, and TalkBack reads it as a switch. */
@Composable
internal fun SwitchRow(
    label: String,
    checked: Boolean,
    modifier: Modifier = Modifier,
    hint: String? = null,
    enabled: Boolean = true,
    onChange: (Boolean) -> Unit,
) {
    val haptics = haptics()
    SettingRow(
        label = label,
        hint = hint,
        enabled = enabled,
        modifier = modifier.toggleable(
            value = checked,
            enabled = enabled,
            role = Role.Switch,
            onValueChange = { v -> haptics.toggle(v); onChange(v) },
        ),
        trailing = { Switch(checked = checked, onCheckedChange = null, enabled = enabled) },
    )
}

/** A row that opens or does something (a button is its trailing control). */
@Composable
internal fun ActionRow(
    label: String,
    modifier: Modifier = Modifier,
    hint: String? = null,
    onClick: (() -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    SettingRow(
        label = label,
        hint = hint,
        modifier = if (onClick != null) modifier.clickable(role = Role.Button, onClick = onClick) else modifier,
        trailing = trailing,
    )
}

/**
 * One of a few short options. Up to four sit in a segmented strip; more wrap as chips.
 * [description] is what TalkBack reads for the group.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
internal fun <T> Segmented(
    options: List<T>,
    selected: T?,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    description: String,
    modifier: Modifier = Modifier,
) {
    val haptics = haptics()
    val pick = { option: T -> if (option != selected) { haptics.tick(); onSelect(option) } }
    BoxWithConstraints(modifier.fillMaxWidth()) {
    // a strip whose buttons would cut their labels ("Automático" on a 360 dp phone) wraps as chips
    val fitsStrip = options.size <= SEGMENTED_MAX_OPTIONS && maxWidth >= SEGMENT_MIN_WIDTH * options.size
    if (fitsStrip) {
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().semantics { contentDescription = description }) {
            options.forEachIndexed { i, option ->
                SegmentedButton(
                    selected = option == selected,
                    onClick = { pick(option) },
                    shape = SegmentedButtonDefaults.itemShape(i, options.size),
                    label = { Text(label(option), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                )
            }
        }
    } else {
        FlowRow(
            Modifier.fillMaxWidth().selectableGroup().semantics { contentDescription = description },
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            options.forEach { option ->
                SelectChip(selected = option == selected, onClick = { pick(option) }, label = { Text(label(option)) })
            }
        }
    }
    }
}

/** A labelled [Segmented] in a stacked row, with the hint of the current choice below. */
@Composable
internal fun <T> SegmentedRow(
    title: String,
    options: List<T>,
    selected: T?,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
    enabled: Boolean = true,
) {
    StackedSetting(title, modifier, hint, enabled) {
        Segmented(options, selected, label, onSelect, description = title)
    }
}

/** A slider with its readout in the label (the caller formats it) and an optional hint. */
@Composable
internal fun SliderRow(
    label: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    onChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    steps: Int = 0,
    hint: String? = null,
    enabled: Boolean = true,
    onFinished: (() -> Unit)? = null,
) {
    StackedSetting(label, modifier, hint, enabled) {
        Slider(value = value, onValueChange = onChange, onValueChangeFinished = onFinished, valueRange = range, steps = steps, enabled = enabled)
    }
}

/** A text field that fills its row; the page decides keyboard, validation and what is saved. */
@Composable
internal fun FieldRow(modifier: Modifier = Modifier, field: @Composable (Modifier) -> Unit) {
    Box(modifier.fillMaxWidth().padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V)) {
        field(Modifier.fillMaxWidth())
    }
}

/** Radio rows with a hint each, for choices that need explaining. */
@Composable
internal fun <T> ChoiceList(
    title: String,
    options: List<T>,
    selected: T,
    label: @Composable (T) -> String,
    hint: @Composable (T) -> String?,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptics = haptics()
    Column(modifier.selectableGroup().semantics { contentDescription = title }) {
        options.forEach { option ->
            val on = option == selected
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = ROW_MIN_HEIGHT)
                    .selectable(selected = on, role = Role.RadioButton) { if (!on) { haptics.tick(); onSelect(option) } }
                    .padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(ROW_PADDING_H),
            ) {
                RowText(label(option), hint(option), enabled = true, modifier = Modifier.weight(1f))
                RadioButton(selected = on, onClick = null)
            }
        }
    }
}

/** How many cards of at least [min] fit in [available] (the rule of GridCells.Adaptive). */
internal fun adaptiveColumns(available: Dp, min: Dp, gap: Dp): Int =
    ((available + gap) / (min + gap)).toInt().coerceAtLeast(1)

/**
 * A grid of picture cards for visual choices (theme, progress bar). It lays out like
 * GridCells.Adaptive(150.dp) but inside the page's own scroll, where a lazy grid can't live.
 */
@Composable
internal fun <T> OptionGallery(
    options: List<T>,
    selected: T,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    minCard: Dp = GALLERY_CARD_MIN,
    preview: @Composable (T) -> Unit,
) {
    val haptics = haptics()
    BoxWithConstraints(modifier.fillMaxWidth().padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V)) {
        val columns = adaptiveColumns(maxWidth, minCard, GALLERY_GAP)
        Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(GALLERY_GAP)) {
            options.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(GALLERY_GAP)) {
                    row.forEach { option ->
                        ChoiceCard(
                            selected = option == selected,
                            label = label(option),
                            onClick = { if (option != selected) { haptics.tick(); onSelect(option) } },
                            modifier = Modifier.weight(1f),
                        ) { preview(option) }
                    }
                    // the last row keeps its cards the same width as the others
                    repeat(columns - row.size) { Box(Modifier.weight(1f)) }
                }
            }
        }
    }
}

/** A picture with its name; selected = primary frame plus a check (not colour alone). */
@Composable
internal fun ChoiceCard(
    selected: Boolean,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    preview: @Composable () -> Unit,
) {
    val scheme = MaterialTheme.colorScheme
    val frame by animateColorAsState(if (selected) scheme.primary else scheme.outlineVariant, LocalMotion.current.color(), label = "card-frame")
    Card(
        modifier = modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onClick),
        shape = MaterialTheme.shapes.medium,
        colors = CardDefaults.cardColors(containerColor = if (selected) scheme.secondaryContainer else scheme.surface),
        border = BorderStroke(if (selected) 2.dp else 1.dp, frame),
    ) {
        Box(Modifier.padding(12.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                preview()
                Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (selected) {
                Icon(
                    Icons.Rounded.CheckCircle,
                    contentDescription = null,
                    tint = scheme.primary,
                    modifier = Modifier.size(20.dp).align(Alignment.TopEnd),
                )
            }
        }
    }
}

/** Explains why something is unavailable or locked; sits at the top of its group. */
@Composable
internal fun SettingsNote(text: String, modifier: Modifier = Modifier, error: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = if (error) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(horizontal = ROW_PADDING_H, vertical = ROW_PADDING_V),
    )
}
