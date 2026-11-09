package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.HomifitoMode
import com.perseusoft.homefi.core.HomifitoPlace

/**
 * Homifito DJ lives in the core's DJ settings (`homifitoMode`, `homifitoPlace`, `homifitoLook`), so it
 * travels with the setups. The only phone-side bit is which size the one-tap button turns back on.
 */
fun SettingsStore.setHomifitoMode(mode: HomifitoMode) {
    setDj(dj.value.copy(homifitoMode = mode))
    if (mode != HomifitoMode.OFF) update { it.copy(djHomifitoBig = mode == HomifitoMode.BIG) }
}

fun SettingsStore.setHomifitoPlace(place: HomifitoPlace) = setDj(dj.value.copy(homifitoPlace = place))

fun SettingsStore.setHomifitoLook(look: Boolean) = setDj(dj.value.copy(homifitoLook = look))

/** The top bar's one tap: off, or back to the size he had. */
fun SettingsStore.toggleHomifito() {
    val on = dj.value.homifitoMode != HomifitoMode.OFF
    setHomifitoMode(if (on) HomifitoMode.OFF else if (settings.value.djHomifitoBig) HomifitoMode.BIG else HomifitoMode.BOOTH)
}
