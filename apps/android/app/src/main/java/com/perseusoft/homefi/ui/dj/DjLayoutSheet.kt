package com.perseusoft.homefi.ui.dj

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.foundation.horizontalScroll
import com.perseusoft.homefi.core.HomifitoMode
import com.perseusoft.homefi.core.HomifitoPlace
import com.perseusoft.homefi.data.setHomifitoLook
import com.perseusoft.homefi.data.setHomifitoMode
import com.perseusoft.homefi.data.setHomifitoPlace
import com.perseusoft.homefi.ui.SelectChip
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.DjLayout
import com.perseusoft.homefi.core.DjSection
import com.perseusoft.homefi.core.DjSectionKind
import com.perseusoft.homefi.core.SfxKind
import com.perseusoft.homefi.core.djDefaultLayout
import com.perseusoft.homefi.data.DEFAULT_DJ_PADS
import com.perseusoft.homefi.data.DJ_PADS
import com.perseusoft.homefi.data.djLayout
import com.perseusoft.homefi.data.setDjLayout
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Card

private val ROW_HEIGHT = 56.dp
private val TARGET = 48.dp

@Composable
fun DjSectionKind.label(): String = stringResource(
    when (this) {
        DjSectionKind.DECK_A -> R.string.djs_section_deck_a
        DjSectionKind.MIXER -> R.string.djs_section_mixer
        DjSectionKind.DECK_B -> R.string.djs_section_deck_b
        DjSectionKind.SAMPLER -> R.string.djs_section_sampler
        DjSectionKind.CRATE -> R.string.djs_section_crate
    },
)

/**
 * "Edit layout": the order of the DJ screen's sections (up / down), which ones show, and the sound of each
 * of the pads. Every change applies at once (the screen behind redraws) and is saved with the DJ settings;
 * a named setup can capture it. The rules (every section once, the pad count) are the core's.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DjLayoutSheet(onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val prefs by app.settings.settings.collectAsStateWithLifecycle()
    val layout = remember(prefs.djSections, prefs.djPads) { prefs.djLayout() }
    val haptics = haptics()
    val apply: (DjLayout) -> Unit = app.settings::setDjLayout

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(stringResource(R.string.djs_edit_layout), style = MaterialTheme.typography.titleLarge)
            Text(
                stringResource(R.string.djs_layout_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Text(
                stringResource(R.string.djs_layout_sections),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.padding(top = 12.dp),
            )
            layout.sections.forEachIndexed { index, section ->
                SectionRow(
                    section = section,
                    canUp = index > 0,
                    canDown = index < layout.sections.lastIndex,
                    onUp = { haptics.tick(); apply(layout.copy(sections = layout.sections.moved(index, index - 1))) },
                    onDown = { haptics.tick(); apply(layout.copy(sections = layout.sections.moved(index, index + 1))) },
                    onVisible = { visible ->
                        apply(layout.copy(sections = layout.sections.mapIndexed { i, s -> if (i == index) s.copy(visible = visible) else s }))
                    },
                )
            }

            HomifitoOptions()

            Text(
                stringResource(R.string.djs_layout_pads),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.padding(top = 12.dp),
            )
            Text(
                stringResource(R.string.djs_layout_pads_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            repeat(DJ_PADS) { pad ->
                PadRow(pad, layout.pads[pad]) { kind ->
                    apply(layout.copy(pads = layout.pads.mapIndexed { i, k -> if (i == pad) kind else k }))
                }
            }

            OutlinedButton(
                onClick = {
                    // the phone's own pads first (as before layouts existed), the core's order for the sections
                    apply(DjLayout(djDefaultLayout().sections, DEFAULT_DJ_PADS))
                },
                modifier = Modifier.fillMaxWidth().heightIn(min = TARGET).padding(top = 12.dp),
            ) { Text(stringResource(R.string.djs_layout_reset)) }
            TextButton(onClick = onDismiss, modifier = Modifier.align(Alignment.End).heightIn(min = TARGET)) {
                Text(stringResource(R.string.dj_ok))
            }
        }
    }
}

@Composable
private fun SectionRow(
    section: DjSection,
    canUp: Boolean,
    canDown: Boolean,
    onUp: () -> Unit,
    onDown: () -> Unit,
    onVisible: (Boolean) -> Unit,
) {
    val label = section.kind.label()
    Card(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
    Row(Modifier.fillMaxWidth().heightIn(min = ROW_HEIGHT).padding(start = 16.dp, end = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (section.visible) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = onUp, enabled = canUp, modifier = Modifier.size(TARGET)) {
            Icon(Icons.Rounded.KeyboardArrowUp, stringResource(R.string.djs_move_up, label))
        }
        IconButton(onClick = onDown, enabled = canDown, modifier = Modifier.size(TARGET)) {
            Icon(Icons.Rounded.KeyboardArrowDown, stringResource(R.string.djs_move_down, label))
        }
        Switch(
            checked = section.visible,
            onCheckedChange = onVisible,
            modifier = Modifier.padding(start = 4.dp),
        )
    }
    }
}

/** One pad: its number and a menu of the eight sounds, each with its picture. */
@Composable
private fun PadRow(pad: Int, current: SfxKind, onPick: (SfxKind) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Card(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
    Row(Modifier.fillMaxWidth().heightIn(min = ROW_HEIGHT).padding(start = 16.dp, end = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.dj_set_pad_n, pad + 1), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        Box {
            OutlinedButton(onClick = { open = true }, modifier = Modifier.heightIn(min = TARGET)) {
                Icon(current.icon(), null, Modifier.size(18.dp))
                Text(stringResource(current.label()), Modifier.padding(start = 8.dp))
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                SfxKind.entries.forEach { kind ->
                    DropdownMenuItem(
                        text = { Text(stringResource(kind.label())) },
                        leadingIcon = { Icon(kind.icon(), null) },
                        onClick = { open = false; onPick(kind) },
                        modifier = Modifier.heightIn(min = TARGET),
                    )
                }
            }
        }
    }
    }
}

/** [this] with the item at [from] moved to [to] (the others keep their order). */
private fun <T> List<T>.moved(from: Int, to: Int): List<T> =
    toMutableList().also { list -> list.add(to.coerceIn(0, list.lastIndex), list.removeAt(from)) }

/** Homifito's size, place and colours (the same options as Settings → DJ), as chips. */
@Composable
private fun HomifitoOptions() {
    val store = LocalContext.current.homefi.settings
    val dj by store.dj.collectAsStateWithLifecycle()
    Text(stringResource(R.string.dj_homifito_group), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(top = 12.dp))
    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        HomifitoMode.entries.forEach { mode ->
            SelectChip(dj.homifitoMode == mode, { store.setHomifitoMode(mode) }, { Text(stringResource(mode.labelRes())) }, Modifier.heightIn(min = TARGET))
        }
    }
    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        HomifitoPlace.entries.forEach { place ->
            SelectChip(dj.homifitoPlace == place, { store.setHomifitoPlace(place) }, { Text(stringResource(place.labelRes())) }, Modifier.heightIn(min = TARGET))
        }
        SelectChip(dj.homifitoLook, { store.setHomifitoLook(!dj.homifitoLook) }, { Text(stringResource(R.string.dj_homifito_look)) }, Modifier.heightIn(min = TARGET))
    }
}
