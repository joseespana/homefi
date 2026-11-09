package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.DjLayout
import com.perseusoft.homefi.core.DjSection
import com.perseusoft.homefi.core.djLayoutClamped

/**
 * The DJ screen's layout as the core defines it (sections in order and visibility, a sound per pad).
 * The settings keep it in two places (the order and the pads); the core completes and cuts it to size,
 * so a layout from an older version, another device or a damaged value is always valid.
 */
fun AppSettings.djLayout(): DjLayout = djLayoutClamped(DjLayout(sections = djSections, pads = djPads))

/** Applies [layout] (clamped by the core) as the current DJ layout. */
fun SettingsStore.setDjLayout(layout: DjLayout) {
    val clamped = djLayoutClamped(layout)
    update { it.copy(djSections = clamped.sections, djPads = clamped.pads) }
}

/** The sections the screen draws, in order. */
fun DjLayout.visibleSections(): List<DjSection> = sections.filter { it.visible }
